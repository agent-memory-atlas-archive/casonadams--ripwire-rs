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
    let Some(repo) = discover(root) else {
        return Vec::new();
    };
    let Ok(iter) = repo.status(gix::progress::Discard).and_then(|s| {
        s.untracked_files(gix::status::UntrackedFiles::None)
            .into_iter(None)
    }) else {
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

/// Lines containing `needle` in the text blobs of `rev`'s tree, path-sorted (`git grep -n -F`).
#[must_use]
pub fn grep_rev(repo: &gix::Repository, rev: &str, needle: &str, limit: usize) -> Vec<LineHit> {
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
    let mut blobs: Vec<_> = recorder
        .records
        .into_iter()
        .filter(|e| e.mode.is_blob())
        .collect();
    blobs.sort_unstable_by(|a, b| a.filepath.cmp(&b.filepath));
    let mut hits = Vec::new();
    for entry in blobs {
        let Ok(blob) = repo.find_blob(entry.oid) else {
            continue;
        };
        if is_binary(&blob.data) {
            continue;
        }
        for (idx, line) in blob.data.lines().enumerate() {
            if line.find(needle).is_some() {
                hits.push(LineHit {
                    path: entry.filepath.to_string(),
                    line: idx + 1,
                    text: line.to_str_lossy().into_owned(),
                });
                if hits.len() == limit {
                    return hits;
                }
            }
        }
    }
    hits
}

fn is_binary(data: &[u8]) -> bool {
    data[..data.len().min(BINARY_PROBE_LEN)].contains(&0)
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
