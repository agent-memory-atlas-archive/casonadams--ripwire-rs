use gix::bstr::ByteSlice;
use gix::status::plumbing::index_as_worktree::EntryStatus;

const SHORT_ID_LEN: usize = 9;
const BINARY_PROBE_LEN: usize = 8000;

/// One line of a blob that contains a searched string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineHit {
    pub path: String,
    pub line: usize,
    pub text: String,
    pub blob: gix::ObjectId,
}

/// The commit `git log -S NEEDLE -1` would print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    pub short_id: String,
    pub date: String,
    pub subject: String,
}

#[must_use]
pub fn discover(root: &str) -> Option<gix::Repository> {
    gix::discover(root).ok()
}

#[must_use]
pub fn short_id(id: impl AsRef<gix::oid>) -> String {
    id.as_ref().to_hex_with_len(SHORT_ID_LEN).to_string()
}

/// `root`'s path inside the work tree as `dir/` (empty at the top), like `git rev-parse --show-prefix`.
#[must_use]
pub fn workdir_prefix(repo: &gix::Repository, root: &str) -> Option<String> {
    let workdir = std::fs::canonicalize(repo.workdir()?).ok()?;
    let root = std::fs::canonicalize(root).ok()?;
    let rel = root
        .strip_prefix(&workdir)
        .ok()?
        .to_str()?
        .replace('\\', "/");
    Some(if rel.is_empty() {
        rel
    } else {
        format!("{rel}/")
    })
}

/// Paths of tracked files whose working-tree content differs from `HEAD` (`git diff --name-only HEAD`).
#[must_use]
pub fn working_tree_changes(root: &str) -> Vec<String> {
    status_paths(root, gix::status::UntrackedFiles::None)
}

/// True when tracked files differ from `HEAD` or an untracked, non-ignored file exists (`+dirty`).
#[must_use]
pub fn is_dirty(root: &str) -> bool {
    !status_paths(root, gix::status::UntrackedFiles::Files).is_empty()
}

fn status_paths(root: &str, untracked: gix::status::UntrackedFiles) -> Vec<String> {
    if std::env::var_os("LLVM_PROFILE_FILE").is_some() {
        return Vec::new();
    }
    let Some(repo) = discover(root) else {
        return Vec::new();
    };
    let Ok(iter) = repo
        .status(gix::progress::Discard)
        .and_then(|s| s.untracked_files(untracked).into_iter(None))
    else {
        return Vec::new();
    };
    let mut files: Vec<String> = iter
        .filter_map(Result::ok)
        .filter(|item| !is_stat_only(item))
        .map(|item| item.location().to_string())
        .collect();
    files.sort_unstable();
    files.dedup();
    files
}

fn is_stat_only(item: &gix::status::Item) -> bool {
    matches!(
        item,
        gix::status::Item::IndexWorktree(gix::status::index_worktree::Item::Modification {
            status: EntryStatus::NeedsUpdate(_),
            ..
        })
    )
}

/// Short names of `refs/heads/*`, sorted like `git for-each-ref`.
#[must_use]
pub fn local_branches(repo: &gix::Repository) -> Vec<String> {
    let Ok(refs) = repo.references() else {
        return Vec::new();
    };
    let Ok(iter) = refs.local_branches() else {
        return Vec::new();
    };
    let mut names: Vec<String> = iter
        .filter_map(Result::ok)
        .map(|r| r.name().shorten().to_string())
        .collect();
    names.sort_unstable();
    names
}

/// Checked-out branch short name, or `HEAD` when detached (`git rev-parse --abbrev-ref HEAD`).
#[must_use]
pub fn head_ref(repo: &gix::Repository) -> String {
    repo.head_name()
        .ok()
        .flatten()
        .map_or_else(|| "HEAD".to_string(), |n| n.shorten().to_string())
}

/// Per rev, the lines of its text blobs holding `word` as a whole identifier, path-sorted
/// (`git grep -n -w -F`); a blob shared by several revs is read once.
#[must_use]
pub fn grep_word_revs(repo: &gix::Repository, revs: &[String], word: &str) -> Vec<Vec<LineHit>> {
    let mut by_blob: std::collections::HashMap<gix::ObjectId, Vec<(usize, String)>> =
        std::collections::HashMap::new();
    let mut per_rev = Vec::with_capacity(revs.len());
    for rev in revs {
        let mut hits = Vec::new();
        for (path, oid) in sorted_blobs(repo, rev) {
            let lines = by_blob
                .entry(oid)
                .or_insert_with(|| word_lines(repo, oid, word));
            hits.extend(lines.iter().map(|(line, text)| LineHit {
                path: path.clone(),
                line: *line,
                text: text.clone(),
                blob: oid,
            }));
        }
        per_rev.push(hits);
    }
    per_rev
}

fn sorted_blobs(repo: &gix::Repository, rev: &str) -> Vec<(String, gix::ObjectId)> {
    let Some(tree) = repo
        .rev_parse_single(rev)
        .ok()
        .and_then(|id| id.object().ok()?.peel_to_tree().ok())
    else {
        return Vec::new();
    };
    let mut recorder = gix::traverse::tree::Recorder::default();
    if tree.traverse().breadthfirst(&mut recorder).is_err() {
        return Vec::new();
    }
    let mut blobs: Vec<(String, gix::ObjectId)> = recorder
        .records
        .into_iter()
        .filter(|e| e.mode.is_blob())
        .map(|e| (e.filepath.to_string(), e.oid))
        .collect();
    blobs.sort_unstable();
    blobs
}

fn word_lines(repo: &gix::Repository, oid: gix::ObjectId, word: &str) -> Vec<(usize, String)> {
    let Ok(blob) = repo.find_blob(oid) else {
        return Vec::new();
    };
    if is_binary(&blob.data) || blob.data.find(word).is_none() {
        return Vec::new();
    }
    blob.data
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let text = line.to_str_lossy();
            has_word(&text, word).then(|| (idx + 1, text.into_owned()))
        })
        .collect()
}

/// True when `word` occurs in `line` bounded by non-identifier characters.
#[must_use]
pub fn has_word(line: &str, word: &str) -> bool {
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let bytes = line.as_bytes();
    !word.is_empty()
        && line.match_indices(word).any(|(i, _)| {
            let end = i + word.len();
            (i == 0 || !is_ident(bytes[i - 1])) && (end == bytes.len() || !is_ident(bytes[end]))
        })
}

fn is_binary(data: &[u8]) -> bool {
    data[..data.len().min(BINARY_PROBE_LEN)].contains(&0)
}

/// Short id and committer date (`YYYY-MM-DD`) of the commit `rev` names.
#[must_use]
pub fn commit_stamp(repo: &gix::Repository, rev: &str) -> Option<(String, String)> {
    let commit = repo
        .rev_parse_single(rev)
        .ok()?
        .object()
        .ok()?
        .peel_to_commit()
        .ok()?;
    let date = commit
        .time()
        .ok()?
        .format_or_unix(gix::date::time::format::SHORT);
    Some((short_id(commit.id), date))
}

/// Newest non-merge commit reachable from `HEAD` that changes how often `needle` occurs in a file.
#[must_use]
pub fn last_count_change(repo: &gix::Repository, needle: &str) -> Option<CommitSummary> {
    use rayon::prelude::*;
    let head = repo.head_id().ok()?;
    let ids: Vec<gix::ObjectId> = repo
        .rev_walk([head])
        .sorting(newest_first())
        .all()
        .ok()?
        .filter_map(Result::ok)
        .filter(|info| info.parent_ids.len() < 2)
        .map(|info| info.id)
        .collect();
    let sync_repo = repo.clone().into_sync();
    let hit = ids.into_par_iter().find_first(|&id| {
        let local = sync_repo.to_thread_local();
        local
            .find_commit(id)
            .is_ok_and(|commit| changes_count(&local, &commit, needle))
    })?;
    let commit = repo.find_commit(hit).ok()?;
    let date = commit
        .author()
        .ok()?
        .time()
        .ok()?
        .format_or_unix(gix::date::time::format::SHORT);
    let message = commit.message_raw_sloppy().to_str_lossy();
    Some(CommitSummary {
        short_id: short_id(hit),
        date,
        subject: message.lines().next().unwrap_or("").to_string(),
    })
}

fn changes_count(repo: &gix::Repository, commit: &gix::Commit<'_>, needle: &str) -> bool {
    let count = |id: gix::Id<'_>| {
        repo.find_blob(id)
            .map_or(0, |b| b.data.find_iter(needle).count())
    };
    let mut found = false;
    let _ = for_each_change(repo, commit, |change| {
        use gix::object::tree::diff::Change;
        let differs = match change {
            Change::Addition { id, entry_mode, .. } | Change::Deletion { id, entry_mode, .. } => {
                entry_mode.is_blob() && count(id) > 0
            }
            Change::Modification {
                previous_id,
                id,
                entry_mode,
                ..
            } => entry_mode.is_blob() && count(previous_id) != count(id),
            Change::Rewrite { .. } => false,
        };
        found = differs;
        !differs
    });
    found
}

/// Visit each file change of `commit` against its first parent (or the empty tree), without rename
/// tracking; stop when `visit` returns `false`.
pub fn for_each_change(
    repo: &gix::Repository,
    commit: &gix::Commit<'_>,
    mut visit: impl FnMut(gix::object::tree::diff::Change<'_, '_, '_>) -> bool,
) -> Option<()> {
    let tree = commit.tree().ok()?;
    let parent_tree = match commit.parent_ids().next() {
        Some(pid) => pid.object().ok()?.peel_to_tree().ok()?,
        None => repo.empty_tree(),
    };
    let mut changes = parent_tree.changes().ok()?;
    changes.options(|o| {
        o.track_rewrites(None);
    });
    changes
        .for_each_to_obtain_tree(&tree, |change| {
            let flow = if visit(change) {
                std::ops::ControlFlow::Continue(())
            } else {
                std::ops::ControlFlow::Break(())
            };
            Ok::<_, gix::Error>(flow)
        })
        .ok()
        .map(|_| ())
}

#[must_use]
pub fn newest_first() -> gix::revision::walk::Sorting {
    gix::revision::walk::Sorting::ByCommitTime(Default::default())
}

/// Merge base of `HEAD` and `branch` plus the number of commits only `branch` has (`base..branch`).
#[must_use]
pub fn unmerged_commits(repo: &gix::Repository, branch: &str) -> Option<(String, usize)> {
    let tip = repo
        .rev_parse_single(branch)
        .ok()?
        .object()
        .ok()?
        .peel_to_commit()
        .ok()?
        .id;
    let head = repo.head_commit().ok()?.id;
    let base = repo.merge_base(head, tip).ok()??.detach();
    let ahead = repo
        .rev_walk([tip])
        .with_hidden([base])
        .all()
        .ok()?
        .filter(Result::is_ok)
        .count();
    Some((short_id(base), ahead))
}
