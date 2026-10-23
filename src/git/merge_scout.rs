//! Git reads for `--merge-scout`, in-process through gix: each arm's merge base with HEAD, the
//! blobs that differ from that base, and the symbols their changed lines fall in.
use super::repo::{is_dirty, revision_commit, short_id, working_tree_changes};
use crate::gate::merge_scout::{ArmChanges, ChangedFiles, touched_symbol_names};
use crate::ingest::pipeline::file_definitions;
use gix::diff::blob::{Algorithm, Diff, InternedInput, sources::byte_lines};
use std::collections::BTreeSet;
use std::ops::Range;

/// Label of the implicit arm that carries a dirty working tree's tracked changes.
const WORKING_TREE: &str = "working-tree";

/// HEAD and the working tree's identity, plus one [`ArmChanges`] per requested ref in order.
pub struct ScoutInputs {
    pub head: String,
    pub at: String,
    pub arms: Vec<ArmChanges>,
}

/// HEAD as the arms are compared against it.
struct HeadState<'r> {
    id: gix::ObjectId,
    tree: gix::Tree<'r>,
    short: String,
    at: String,
}

/// A path that differs between two trees; `None` on the side where the blob is absent.
struct FileDelta {
    path: String,
    old: Option<gix::ObjectId>,
    new: Option<gix::ObjectId>,
}

/// Resolve each ref against HEAD and collect what it changed since the merge base.
///
/// A dirty working tree is one more arm, last, so its uncommitted work is scouted with the refs.
/// Errors on an unknown ref or an unborn HEAD. An arm that shares no history with HEAD is returned
/// with `base: None` so the report can say it did not run.
pub fn collect_scout(root: &str, refs: &[&str]) -> Result<ScoutInputs, String> {
    let repo = super::repo::discover(root).ok_or("not a git repository")?;
    let head = read_head(&repo, root)?;
    let mut arms = refs
        .iter()
        .map(|&label| collect_arm(&repo, &head, label))
        .collect::<Result<Vec<_>, _>>()?;
    if is_dirty(root) {
        arms.push(working_tree_arm(&repo, root, &head));
    }
    Ok(ScoutInputs {
        head: head.short,
        at: head.at,
        arms,
    })
}

fn read_head<'r>(repo: &'r gix::Repository, root: &str) -> Result<HeadState<'r>, String> {
    let commit = repo
        .head_commit()
        .map_err(|e| format!("HEAD has no commit: {e}"))?;
    let tree = commit.tree().map_err(|e| format!("HEAD tree: {e}"))?;
    let short = short_id(commit.id);
    let at = format!("{short}{}", at_suffix(repo, root));
    Ok(HeadState {
        id: commit.id,
        tree,
        short,
        at,
    })
}

/// `+dirty` for a dirty working tree and `+shallow` for a truncated history, where merge bases
/// may be missing.
fn at_suffix(repo: &gix::Repository, root: &str) -> String {
    let mut suffix = String::new();
    if is_dirty(root) {
        suffix.push_str("+dirty");
    }
    if repo.is_shallow().unwrap_or(false) {
        suffix.push_str("+shallow");
    }
    suffix
}

fn collect_arm(
    repo: &gix::Repository,
    head: &HeadState<'_>,
    label: &str,
) -> Result<ArmChanges, String> {
    let tip = revision_commit(repo, label).ok_or_else(|| format!("unknown ref '{label}'"))?;
    let Some(base) = merge_base(repo, head.id, tip.id) else {
        return Ok(unrelated_arm(label));
    };
    let base_tree = commit_tree(repo, base).map_err(|e| format!("merge-base of '{label}': {e}"))?;
    let arm_tree = tip.tree().map_err(|e| format!("tree of '{label}': {e}"))?;
    Ok(ArmChanges {
        label: label.to_string(),
        base: Some(short_id(base)),
        files: changed_since(repo, &base_tree, &arm_tree),
        head_files: changed_since(repo, &base_tree, &head.tree),
    })
}

/// The uncommitted tracked changes as an arm, against HEAD: the old side is HEAD's blob, the new
/// side the file on disk (empty when deleted). Untracked files are not work, so an arm of only
/// untracked files reports no work.
fn working_tree_arm(repo: &gix::Repository, root: &str, head: &HeadState<'_>) -> ArmChanges {
    let mut files = ChangedFiles::new();
    for path in working_tree_changes(root) {
        let old = blob_bytes(repo, head_blob(&head.tree, &path));
        let new = repo
            .workdir()
            .map(|dir| std::fs::read(dir.join(&path)).unwrap_or_default())
            .unwrap_or_default();
        files.insert(path.clone(), symbols_between(&path, &old, &new));
    }
    ArmChanges {
        label: WORKING_TREE.to_string(),
        base: Some(head.short.clone()),
        files,
        head_files: ChangedFiles::new(),
    }
}

fn head_blob(tree: &gix::Tree<'_>, path: &str) -> Option<gix::ObjectId> {
    let entry = tree.lookup_entry_by_path(path).ok().flatten()?;
    entry.mode().is_blob().then(|| entry.object_id())
}

fn unrelated_arm(label: &str) -> ArmChanges {
    ArmChanges {
        label: label.to_string(),
        base: None,
        files: ChangedFiles::new(),
        head_files: ChangedFiles::new(),
    }
}

fn merge_base(repo: &gix::Repository, a: gix::ObjectId, b: gix::ObjectId) -> Option<gix::ObjectId> {
    repo.merge_base(a, b).ok().flatten().map(gix::Id::detach)
}

fn commit_tree(repo: &gix::Repository, id: gix::ObjectId) -> Result<gix::Tree<'_>, String> {
    let commit = repo.find_commit(id).map_err(|e| e.to_string())?;
    commit.tree().map_err(|e| e.to_string())
}

/// Every blob path that differs from `from` to `to`, with the symbols its changed lines touch.
fn changed_since(repo: &gix::Repository, from: &gix::Tree<'_>, to: &gix::Tree<'_>) -> ChangedFiles {
    tree_deltas(from, to)
        .into_iter()
        .map(|delta| {
            let names = delta_symbols(repo, &delta);
            (delta.path, names)
        })
        .collect()
}

fn tree_deltas(from: &gix::Tree<'_>, to: &gix::Tree<'_>) -> Vec<FileDelta> {
    use gix::object::tree::diff::Change;
    let mut deltas = Vec::new();
    let Ok(mut changes) = from.changes() else {
        return deltas;
    };
    changes.options(|o| {
        o.track_rewrites(None);
    });
    let _ = changes.for_each_to_obtain_tree(to, |change| {
        match change {
            Change::Addition {
                location,
                id,
                entry_mode,
                ..
            } if entry_mode.is_blob() => deltas.push(FileDelta {
                path: location.to_string(),
                old: None,
                new: Some(id.detach()),
            }),
            Change::Deletion {
                location,
                id,
                entry_mode,
                ..
            } if entry_mode.is_blob() => deltas.push(FileDelta {
                path: location.to_string(),
                old: Some(id.detach()),
                new: None,
            }),
            Change::Modification {
                location,
                previous_id,
                id,
                entry_mode,
                ..
            } if entry_mode.is_blob() => deltas.push(FileDelta {
                path: location.to_string(),
                old: Some(previous_id.detach()),
                new: Some(id.detach()),
            }),
            _ => {}
        }
        Ok::<_, gix::Error>(std::ops::ControlFlow::Continue(()))
    });
    deltas
}

/// Symbols touched by one committed file change.
fn delta_symbols(repo: &gix::Repository, delta: &FileDelta) -> BTreeSet<String> {
    let old = blob_bytes(repo, delta.old);
    let new = blob_bytes(repo, delta.new);
    symbols_between(&delta.path, &old, &new)
}

/// Symbols touched on either side of one file's change: old-side lines name what was removed or
/// rewritten, new-side lines what was added or rewritten.
fn symbols_between(path: &str, old: &[u8], new: &[u8]) -> BTreeSet<String> {
    let (old_lines, new_lines) = changed_lines(old, new);
    let mut names = touched_symbol_names(old, &file_definitions(path, old), &old_lines);
    names.extend(touched_symbol_names(
        new,
        &file_definitions(path, new),
        &new_lines,
    ));
    names
}

/// Blob contents; empty when absent, unreadable, or binary (binary files have no symbols).
fn blob_bytes(repo: &gix::Repository, id: Option<gix::ObjectId>) -> Vec<u8> {
    let Some(id) = id else {
        return Vec::new();
    };
    match repo.find_blob(id) {
        Ok(blob) if !crate::crawler::is_binary_buffer(&blob.data) => blob.data.to_vec(),
        _ => Vec::new(),
    }
}

/// Changed line ranges (0-based, half-open) on the old and new side of a line diff.
fn changed_lines(old: &[u8], new: &[u8]) -> (Vec<Range<u32>>, Vec<Range<u32>>) {
    let input = InternedInput::new(byte_lines(old), byte_lines(new));
    let diff = Diff::compute(Algorithm::Histogram, &input);
    diff.hunks().map(|h| (h.before, h.after)).unzip()
}
