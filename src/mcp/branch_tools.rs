use crate::domain::{IngestResult, Interner};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// Execute `owners` tool: git commit author ownership per file.
fn record_commit_authors(
    file_authors: &mut HashMap<String, HashMap<String, f64>>,
    files: &[String],
    author: &str,
    weight: f64,
    path_filter: Option<&str>,
    known_files: Option<&std::collections::HashSet<String>>,
) {
    for f in files {
        if let Some(known) = known_files
            && !known.contains(f)
        {
            continue;
        }
        if path_filter.is_none() || path_filter.is_some_and(|pf| f.contains(pf)) {
            let entry = file_authors.entry(f.clone()).or_default();
            *entry.entry(author.to_string()).or_insert(0.0) += weight;
        }
    }
}

/// Execute `owners` tool: git commit author ownership per file.
fn compute_file_author_weights(
    commits: &[crate::git::history::CommitInfo],
    path_filter: Option<&str>,
    known_files: Option<&std::collections::HashSet<String>>,
) -> HashMap<String, HashMap<String, f64>> {
    let mut file_authors: HashMap<String, HashMap<String, f64>> = HashMap::new();
    let head_epoch = commits.first().map_or(0, |c| c.timestamp);

    for c in commits {
        let age_sec = (head_epoch - c.timestamp).max(0);
        let age_days = (age_sec as f64) / 86400.0;
        let weight = 0.5f64.powf(age_days / 180.0);
        let author = if c.author.is_empty() {
            "unknown"
        } else {
            &c.author
        };
        record_commit_authors(
            &mut file_authors,
            &c.changed_files,
            author,
            weight,
            path_filter,
            known_files,
        );
    }
    file_authors
}

fn score_file_ownership(f: String, author_weights: HashMap<String, f64>) -> Value {
    let authors_count = author_weights.len();
    let total_weight: f64 = author_weights.values().sum();
    let (top_author, top_weight) = author_weights
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or_default();
    let share = if total_weight > 0.0 {
        top_weight / total_weight
    } else {
        1.0
    };
    let bf = if share >= 0.80 { 1 } else { 0 };

    json!({
        "file": f.clone(),
        "path": f,
        "authors": authors_count,
        "bf": bf,
        "top": top_author.clone(),
        "top_author": top_author,
        "share": (share * 100.0).round() / 100.0,
        "top_share": (share * 100.0).round() / 100.0,
    })
}

/// Execute `owners` tool: git commit author ownership per file.
pub fn call_owners(
    commits: Option<&[crate::git::history::CommitInfo]>,
    path_filter: Option<&str>,
    at_stamp: Option<&str>,
    known_files: Option<&std::collections::HashSet<String>>,
) -> Value {
    let Some(commits) = commits else {
        return json!({ "error": "git commit history unavailable" });
    };

    let file_authors = compute_file_author_weights(commits, path_filter, known_files);
    let mut file_results: Vec<Value> = file_authors
        .into_iter()
        .map(|(f, map)| score_file_ownership(f, map))
        .collect();

    file_results.sort_by(|a, b| {
        let path_a = a["path"].as_str().unwrap_or("");
        let path_b = b["path"].as_str().unwrap_or("");
        path_a.cmp(path_b)
    });

    json!({
        "files_analyzed": file_results.len(),
        "files": file_results.clone(),
        "ownership": file_results,
        "at": at_stamp
    })
}

fn branch_hit_json(
    branch: &str,
    stamp: &(String, String),
    symbol_name: &str,
    hit: &crate::git::repo::LineHit,
    kind: &str,
) -> Value {
    let code_text = hit.text.trim();
    json!({
        "ref": branch,
        "tip": stamp.0,
        "date": stamp.1,
        "symbol": symbol_name,
        "kind": kind,
        "file": hit.path,
        "path": hit.path,
        "line": hit.line,
        "text": code_text,
        "on_head": false
    })
}

struct WhereisScan {
    rows: Vec<Value>,
    on_head: bool,
    refs_scanned: usize,
    blobs: usize,
}

fn scan_repo_whereis(
    repo: &gix::Repository,
    root: &str,
    symbol_name: &str,
    index_defs: &HashMap<&str, HashSet<u32>>,
) -> WhereisScan {
    let scope = crate::git::repo::workdir_prefix(repo, root).unwrap_or_default();
    let head = repo.head_id().ok().map(gix::Id::detach);
    let branches: Vec<String> = crate::git::repo::local_branches(repo)
        .into_iter()
        .filter(|b| repo.rev_parse_single(b.as_str()).ok().map(gix::Id::detach) != head)
        .collect();
    let revs: Vec<&str> = std::iter::once("HEAD")
        .chain(branches.iter().map(String::as_str))
        .collect();
    let grep = crate::git::repo::grep_word_revs(repo, &revs, &scope, symbol_name);
    let mut per_rev = grep.per_rev.into_iter();
    let head_hits = per_rev.next().unwrap_or_default();
    let changed: Vec<String> = crate::git::repo::dirty_paths(root)
        .into_iter()
        .filter_map(|p| p.strip_prefix(scope.as_str()).map(str::to_string))
        .collect();
    let root_path = std::path::Path::new(root);
    let mut rows: Vec<Value> = head_hits
        .iter()
        .filter(|h| changed.binary_search(&h.path).is_err())
        .map(|h| {
            let src = CheckoutLine {
                label: "HEAD",
                path: &h.path,
                line: h.line,
                text: &h.text,
            };
            checkout_row(&src, symbol_name, index_defs.get(h.path.as_str()))
        })
        .collect();
    for path in &changed {
        let defs = index_defs.get(path.as_str());
        rows.extend(scan_file_whereis_hits(
            root_path,
            path,
            symbol_name,
            defs,
            "worktree",
        ));
    }
    sort_checkout_rows(&mut rows);
    let on_head = !rows.is_empty();
    let per_branch: Vec<Vec<crate::git::repo::LineHit>> = per_rev.collect();
    rows.extend(branch_rows(repo, &branches, &per_branch, symbol_name));
    WhereisScan {
        rows,
        on_head,
        refs_scanned: branches.len(),
        blobs: grep.blobs,
    }
}

fn branch_rows(
    repo: &gix::Repository,
    branches: &[String],
    per_branch: &[Vec<crate::git::repo::LineHit>],
    symbol_name: &str,
) -> Vec<Value> {
    let def_lines = blob_def_lines(repo, per_branch, symbol_name);
    let mut rows = Vec::new();
    for (branch, hits) in branches.iter().zip(per_branch) {
        let stamp = crate::git::repo::commit_stamp(repo, branch).unwrap_or_default();
        rows.extend(hits.iter().map(|hit| {
            let kind = if def_lines[&hit.blob].contains(&(hit.line as u32)) {
                "def"
            } else {
                "ref"
            };
            branch_hit_json(branch, &stamp, symbol_name, hit, kind)
        }));
    }
    rows
}

fn blob_def_lines(
    repo: &gix::Repository,
    per_branch: &[Vec<crate::git::repo::LineHit>],
    symbol_name: &str,
) -> HashMap<gix::ObjectId, Vec<u32>> {
    use rayon::prelude::*;
    let mut first_path: HashMap<gix::ObjectId, &str> = HashMap::new();
    for hit in per_branch.iter().flatten() {
        first_path.entry(hit.blob).or_insert(&hit.path);
    }
    let sync_repo = repo.clone().into_sync();
    first_path
        .into_par_iter()
        .map_init(
            || sync_repo.to_thread_local(),
            |r, (oid, path)| {
                let defs = r.find_blob(oid).map_or_else(
                    |_| Vec::new(),
                    |b| crate::ingest::definition_lines(path, &b.data, symbol_name),
                );
                (oid, defs)
            },
        )
        .collect()
}

pub(super) fn probe_name_history(root: &str, sym_name: &str, head_sha: &str) -> Option<Value> {
    let repo = crate::git::repo::discover(root)?;
    if let Some(commit) = crate::git::repo::last_count_change(&repo, sym_name) {
        return Some(json!({
            "probed": 1,
            "head": head_sha,
            "fate": {
                "v": "removed",
                "commit": commit.short_id,
                "date": commit.date,
                "subject": commit.subject
            }
        }));
    }
    Some(json!({
        "probed": 1,
        "head": head_sha,
        "fate": {
            "v": "never",
            "note": "no commit reachable from HEAD ever removed a line carrying this name — so for a name HEAD does not have, this repo never had it"
        }
    }))
}

struct CheckoutLine<'a> {
    label: &'a str,
    path: &'a str,
    line: usize,
    text: &'a str,
}

fn checkout_row(src: &CheckoutLine<'_>, symbol_name: &str, defs: Option<&HashSet<u32>>) -> Value {
    let line = src.line as u32;
    let is_def = defs.is_some_and(|s| s.contains(&line));
    let code_text: String = src.text.trim().chars().take(200).collect();
    json!({
        "ref": src.label,
        "symbol": symbol_name,
        "kind": if is_def { "def" } else { "ref" },
        "file": src.path,
        "path": src.path,
        "line": line,
        "text": code_text,
        "on_head": true,
        "is_def": is_def,
    })
}

fn scan_file_whereis_hits(
    root_path: &std::path::Path,
    rel_path: &str,
    symbol_name: &str,
    defs: Option<&HashSet<u32>>,
    label: &str,
) -> Vec<Value> {
    let full_path = if root_path == std::path::Path::new(".") {
        std::path::PathBuf::from(rel_path)
    } else {
        root_path.join(rel_path)
    };
    let Ok(content) = std::fs::read_to_string(full_path) else {
        return Vec::new();
    };
    if !content.contains(symbol_name) {
        return Vec::new();
    }
    content
        .lines()
        .enumerate()
        .filter(|(_, text)| crate::git::repo::has_word(text, symbol_name))
        .map(|(idx, text)| {
            let src = CheckoutLine {
                label,
                path: rel_path,
                line: idx + 1,
                text,
            };
            checkout_row(&src, symbol_name, defs)
        })
        .collect()
}

fn scan_files_whereis(
    ingest: &IngestResult,
    interner: &Interner,
    root: &str,
    symbol_name: &str,
    index_defs: &HashMap<&str, HashSet<u32>>,
) -> WhereisScan {
    let root_path = std::path::Path::new(root);
    let mut rows: Vec<Value> = ingest
        .files
        .iter()
        .flat_map(|fi| {
            let path = interner.resolve(&fi.path);
            scan_file_whereis_hits(root_path, path, symbol_name, index_defs.get(path), "HEAD")
        })
        .collect();
    sort_checkout_rows(&mut rows);
    WhereisScan {
        on_head: !rows.is_empty(),
        rows,
        refs_scanned: 0,
        blobs: ingest.files.len(),
    }
}

fn sort_checkout_rows(rows: &mut [Value]) {
    rows.sort_by(|a, b| {
        let a_def = a["is_def"].as_bool().unwrap_or(false);
        let b_def = b["is_def"].as_bool().unwrap_or(false);
        b_def
            .cmp(&a_def)
            .then_with(|| a["path"].as_str().cmp(&b["path"].as_str()))
            .then_with(|| a["line"].as_u64().cmp(&b["line"].as_u64()))
    });
}

#[must_use]
pub fn index_def_lines<'a>(
    ingest: &IngestResult,
    interner: &'a Interner,
    symbol_name: &str,
) -> HashMap<&'a str, HashSet<u32>> {
    let mut defs: HashMap<&str, HashSet<u32>> = HashMap::new();
    for sym in &ingest.symbols {
        if interner.resolve(&sym.name) == symbol_name
            && let Some(file) = ingest.files.get(sym.file_id as usize)
        {
            defs.entry(interner.resolve(&file.path))
                .or_default()
                .insert(sym.line);
        }
    }
    defs
}

/// Execute `whereis` tool: identify where a symbol lives (definitions and references).
pub fn call_whereis(
    ingest: &IngestResult,
    interner: &Interner,
    root: &str,
    symbol_name: &str,
    with_history: bool,
) -> Value {
    let at_stamp = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
    let repo = crate::git::repo::discover(root);
    let head_date = repo
        .as_ref()
        .and_then(|repo| crate::git::repo::commit_stamp(repo, "HEAD"))
        .map(|(_, date)| date)
        .unwrap_or_default();
    let index_defs = index_def_lines(ingest, interner, symbol_name);
    let scan = match &repo {
        Some(repo) => scan_repo_whereis(repo, root, symbol_name, &index_defs),
        None => scan_files_whereis(ingest, interner, root, symbol_name, &index_defs),
    };
    let history_info = if with_history && !scan.on_head {
        probe_name_history(root, symbol_name, &at_stamp)
    } else {
        None
    };

    let hits_count = scan.rows.len();
    json!({
        "symbol": symbol_name,
        "on_head": u8::from(scan.on_head),
        "refs_scanned": scan.refs_scanned,
        "blobs": scan.blobs,
        "hits_count": hits_count,
        "count": hits_count,
        "hits": scan.rows.clone(),
        "locations": scan.rows,
        "history": history_info,
        "head_date": head_date,
        "at": at_stamp
    })
}

pub fn inspect_branch_stray(
    repo: &gix::Repository,
    branch: &str,
    head_ref: &str,
) -> Option<(String, Value)> {
    if branch.is_empty() || branch == "HEAD" || branch == head_ref {
        return None;
    }
    let Some((short_base, unmerged)) = crate::git::repo::unmerged_commits(repo, branch) else {
        return Some(("unknown".to_string(), json!({})));
    };
    if unmerged > 0 {
        Some((
            "unmerged".to_string(),
            json!({
                "name": branch,
                "branch": branch,
                "ok": true,
                "verdict": "unmerged",
                "base": short_base,
                "stray": unmerged
            }),
        ))
    } else {
        Some(("merged".to_string(), json!({})))
    }
}

fn gather_branch_stray(
    repo: &gix::Repository,
    head_ref: &str,
) -> (usize, usize, usize, usize, Vec<Value>) {
    let mut refs_count = 0usize;
    let mut unmerged = 0usize;
    let mut merged = 0usize;
    let mut unknown = 0usize;
    let mut branches = Vec::new();
    for branch in crate::git::repo::local_branches(repo) {
        if let Some((verdict, branch_val)) = inspect_branch_stray(repo, &branch, head_ref) {
            refs_count += 1;
            match verdict.as_str() {
                "unmerged" => {
                    unmerged += 1;
                    branches.push(branch_val);
                }
                "merged" => merged += 1,
                _ => unknown += 1,
            }
        }
    }
    (refs_count, unmerged, merged, unknown, branches)
}

/// Execute `stray_content` tool.
pub fn call_stray_content(root: &str) -> Value {
    let at_stamp = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
    let repo = crate::git::repo::discover(root);
    let head_sha = repo
        .as_ref()
        .and_then(|r| r.head_id().ok())
        .map(crate::git::repo::short_id)
        .unwrap_or_default();
    let head_ref = repo
        .as_ref()
        .map_or_else(|| "HEAD".to_string(), crate::git::repo::head_ref);
    let (refs_count, unmerged, merged, unknown, branches) =
        repo.as_ref().map_or((0, 0, 0, 0, Vec::new()), |r| {
            gather_branch_stray(r, &head_ref)
        });
    json!({
        "head": head_sha,
        "head_ref": head_ref,
        "refs": refs_count,
        "blobs": 0,
        "unmerged": unmerged,
        "superseded": 0,
        "merged": merged,
        "unknown": unknown,
        "at": at_stamp,
        "branches": branches
    })
}
