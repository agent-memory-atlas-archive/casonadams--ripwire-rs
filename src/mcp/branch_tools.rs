use crate::domain::{IngestResult, Interner};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;

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

fn get_line_snippet(path: &str, line: u32, fallback: &str) -> String {
    if line == 0 {
        return fallback.to_string();
    }
    if let Ok(content) = fs::read_to_string(path)
        && let Some(l) = content.lines().nth(line as usize - 1)
    {
        let trimmed = l.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    fallback.to_string()
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

/// Hits for `symbol_name` on every local branch whose tip is not HEAD's commit, labeled `def`
/// where the blob's own tags parse defines the name on that line.
fn scan_local_branch_hits(root: &str, symbol_name: &str) -> (usize, Vec<Value>) {
    let Some(repo) = crate::git::repo::discover(root) else {
        return (0, Vec::new());
    };
    let head = repo.head_id().ok().map(gix::Id::detach);
    let branches: Vec<String> = crate::git::repo::local_branches(&repo)
        .into_iter()
        .filter(|b| repo.rev_parse_single(b.as_str()).ok().map(gix::Id::detach) != head)
        .collect();
    let per_branch = crate::git::repo::grep_word_revs(&repo, &branches, symbol_name);
    let mut def_lines: std::collections::HashMap<gix::ObjectId, Vec<u32>> =
        std::collections::HashMap::new();
    let mut hits = Vec::new();
    for (branch, branch_hits) in branches.iter().zip(per_branch) {
        let stamp = crate::git::repo::commit_stamp(&repo, branch).unwrap_or_default();
        for hit in &branch_hits {
            let defs = def_lines.entry(hit.blob).or_insert_with(|| {
                repo.find_blob(hit.blob).map_or_else(
                    |_| Vec::new(),
                    |b| crate::ingest::definition_lines(&hit.path, &b.data, symbol_name),
                )
            });
            let kind = if defs.contains(&(hit.line as u32)) {
                "def"
            } else {
                "ref"
            };
            hits.push(branch_hit_json(branch, &stamp, symbol_name, hit, kind));
        }
    }
    (branches.len(), hits)
}

fn resolve_hit_file<'a>(ingest: &'a IngestResult, interner: &'a Interner, file_id: u32) -> &'a str {
    ingest
        .files
        .get(file_id as usize)
        .map(|f| interner.resolve(&f.path))
        .unwrap_or("")
}

fn build_head_hit(
    file_path: &str,
    root: &str,
    line: u32,
    sym_name: &str,
    def_kind: Option<crate::domain::SymKind>,
) -> Value {
    let full_path = if root == "." {
        file_path.to_string()
    } else {
        format!("{root}/{file_path}")
    };
    let (kind, fallback) = match def_kind {
        Some(k) => ("def", format!("{} {sym_name}", k.tag())),
        None => ("ref", sym_name.to_string()),
    };
    let text = get_line_snippet(&full_path, line, &fallback);
    json!({
        "ref": "HEAD",
        "symbol": sym_name,
        "kind": kind,
        "file": file_path,
        "path": file_path,
        "line": line,
        "text": text,
        "on_head": true
    })
}

fn collect_head_defs(
    ingest: &IngestResult,
    interner: &Interner,
    root: &str,
    sym_name: &str,
) -> Vec<Value> {
    ingest
        .symbols
        .iter()
        .filter(|sym| interner.resolve(&sym.name) == sym_name)
        .map(|sym| {
            let fp = resolve_hit_file(ingest, interner, sym.file_id);
            build_head_hit(fp, root, sym.line, sym_name, Some(sym.kind))
        })
        .collect()
}

fn collect_head_refs(
    ingest: &IngestResult,
    interner: &Interner,
    root: &str,
    sym_name: &str,
) -> Vec<Value> {
    ingest
        .references
        .iter()
        .filter(|r| interner.resolve(&r.callee_name) == sym_name)
        .map(|r| {
            let fp = resolve_hit_file(ingest, interner, r.file_id);
            build_head_hit(fp, root, r.line, sym_name, None)
        })
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

fn scan_file_whereis_hits(
    root_path: &std::path::Path,
    rel_path: &str,
    symbol_name: &str,
    def_lines: Option<&std::collections::HashSet<u32>>,
) -> Vec<Value> {
    let mut file_hits = Vec::new();
    let full_path = if root_path == std::path::Path::new(".") {
        std::path::PathBuf::from(rel_path)
    } else {
        root_path.join(rel_path)
    };
    let Ok(content) = std::fs::read_to_string(full_path) else {
        return file_hits;
    };
    if !content.contains(symbol_name) {
        return file_hits;
    }
    for (idx, raw_line) in content.lines().enumerate() {
        if !crate::git::repo::has_word(raw_line, symbol_name) {
            continue;
        }
        let line_num = (idx + 1) as u32;
        let is_def = def_lines.is_some_and(|s| s.contains(&line_num));
        let kind = if is_def { "def" } else { "ref" };
        let code_text: String = raw_line.trim().chars().take(200).collect();
        file_hits.push(json!({
            "ref": "HEAD",
            "symbol": symbol_name,
            "kind": kind,
            "file": rel_path,
            "path": rel_path,
            "line": line_num,
            "text": code_text,
            "on_head": true,
            "is_def": is_def,
        }));
    }
    file_hits
}

fn collect_worktree_whereis_hits(
    ingest: &IngestResult,
    interner: &Interner,
    root: &str,
    symbol_name: &str,
    defs_by_file: &std::collections::HashMap<u32, std::collections::HashSet<u32>>,
) -> Vec<Value> {
    let root_path = std::path::Path::new(root);
    let mut hits: Vec<Value> = ingest
        .files
        .iter()
        .flat_map(|fi| {
            let rel_path = interner.resolve(&fi.path);
            scan_file_whereis_hits(root_path, rel_path, symbol_name, defs_by_file.get(&fi.id))
        })
        .collect();
    if hits.is_empty() {
        let mut h = collect_head_defs(ingest, interner, root, symbol_name);
        h.append(&mut collect_head_refs(ingest, interner, root, symbol_name));
        return h;
    }
    hits.sort_by(|a, b| {
        let a_def = a["is_def"].as_bool().unwrap_or(false);
        let b_def = b["is_def"].as_bool().unwrap_or(false);
        b_def
            .cmp(&a_def)
            .then_with(|| a["path"].as_str().cmp(&b["path"].as_str()))
            .then_with(|| a["line"].as_u64().cmp(&b["line"].as_u64()))
    });
    hits
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
    let head_date = crate::git::repo::discover(root)
        .and_then(|repo| crate::git::repo::commit_stamp(&repo, "HEAD"))
        .map(|(_, date)| date)
        .unwrap_or_default();
    let mut defs_by_file: std::collections::HashMap<u32, std::collections::HashSet<u32>> =
        std::collections::HashMap::new();
    for sym in &ingest.symbols {
        if interner.resolve(&sym.name) == symbol_name {
            defs_by_file
                .entry(sym.file_id)
                .or_default()
                .insert(sym.line);
        }
    }

    let mut hits =
        collect_worktree_whereis_hits(ingest, interner, root, symbol_name, &defs_by_file);

    let on_head = !hits.is_empty();
    let blobs = ingest.files.len();
    let (refs_scanned, mut branch_hits) = scan_local_branch_hits(root, symbol_name);
    hits.append(&mut branch_hits);

    let history_info = if with_history && !on_head {
        probe_name_history(root, symbol_name, &at_stamp)
    } else {
        None
    };

    let hits_count = hits.len();
    json!({
        "symbol": symbol_name,
        "on_head": if on_head { 1 } else { 0 },
        "refs_scanned": refs_scanned,
        "blobs": blobs,
        "hits_count": hits_count,
        "count": hits_count,
        "hits": hits.clone(),
        "locations": hits,
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
