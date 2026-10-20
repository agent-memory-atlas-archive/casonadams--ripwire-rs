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

fn strip_decl_modifiers(mut s: &str) -> &str {
    const MODIFIERS: &[&str] = &[
        "pub(crate) ",
        "pub(super) ",
        "pub ",
        "export ",
        "default ",
        "async ",
        "inline ",
        "constexpr ",
        "static ",
        "virtual ",
        "unsafe ",
        "const ",
    ];
    while let Some(&m) = MODIFIERS.iter().find(|&&m| s.starts_with(m)) {
        s = s[m.len()..].trim_start();
    }
    s
}

pub fn classify_code_line(code: &str) -> &'static str {
    const DEF_PREFIXES: &[&str] = &[
        "fn ",
        "func ",
        "function ",
        "def ",
        "class ",
        "struct ",
        "enum ",
        "trait ",
        "interface ",
        "type ",
        "#define ",
    ];
    let stripped = strip_decl_modifiers(code.trim_start());
    if DEF_PREFIXES.iter().any(|&p| stripped.starts_with(p)) {
        "def"
    } else {
        "ref"
    }
}

fn branch_hit_json(branch: &str, symbol_name: &str, hit: &crate::git::repo::LineHit) -> Value {
    let code_text = hit.text.trim();
    json!({
        "ref": branch,
        "symbol": symbol_name,
        "kind": classify_code_line(code_text),
        "file": hit.path,
        "path": hit.path,
        "line": hit.line,
        "text": code_text,
        "on_head": false
    })
}

pub fn should_scan_branch(branch: &str, on_head: bool) -> bool {
    !branch.is_empty() && branch != "HEAD" && !(branch == "main" && on_head)
}

fn scan_local_branch_hits(root: &str, symbol_name: &str, on_head: bool) -> (usize, Vec<Value>) {
    let Some(repo) = crate::git::repo::discover(root) else {
        return (0, Vec::new());
    };
    let mut hits = Vec::new();
    let mut refs_scanned = 0usize;
    for branch in crate::git::repo::local_branches(&repo) {
        if !should_scan_branch(&branch, on_head) {
            continue;
        }
        refs_scanned += 1;
        for hit in crate::git::repo::grep_rev(&repo, &branch, symbol_name, 60) {
            hits.push(branch_hit_json(&branch, symbol_name, &hit));
        }
    }
    (refs_scanned, hits)
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

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn line_contains_word(line: &str, word: &str) -> bool {
    if word.is_empty() || line.len() < word.len() {
        return false;
    }
    let bytes = line.as_bytes();
    let wlen = word.len();
    let mut offset = 0;
    while let Some(pos) = line[offset..].find(word) {
        let idx = offset + pos;
        let before_ok = idx == 0 || !is_word_byte(bytes[idx - 1]);
        let after_ok = idx + wlen >= bytes.len() || !is_word_byte(bytes[idx + wlen]);
        if before_ok && after_ok {
            return true;
        }
        offset = idx + 1;
    }
    false
}

fn head_tip_and_date(root: &str, at_stamp: &str) -> (String, String) {
    let tip = at_stamp
        .strip_suffix("+dirty")
        .unwrap_or(at_stamp)
        .to_string();
    if let Ok(repo) = gix::open(std::path::Path::new(root))
        && let Ok(head) = repo.head_commit()
        && let Ok(time) = head.time()
    {
        let days = time.seconds / 86_400;
        let z = days + 719_468;
        let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
        let doe = (z - era * 146_097) as u32;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = (yoe as i64) + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = if m <= 2 { y + 1 } else { y };
        return (tip, format!("{year:04}-{m:02}-{d:02}"));
    }
    (tip, "1970-01-01".to_string())
}

fn scan_file_whereis_hits(
    root_path: &std::path::Path,
    rel_path: &str,
    symbol_name: &str,
    tip: &str,
    date: &str,
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
        if !line_contains_word(raw_line, symbol_name) {
            continue;
        }
        let line_num = (idx + 1) as u32;
        let is_def = def_lines.is_some_and(|s| s.contains(&line_num));
        let kind = if is_def { "def" } else { "ref" };
        let code_text: String = raw_line.trim().chars().take(200).collect();
        file_hits.push(json!({
            "ref": "HEAD",
            "tip": tip,
            "date": date,
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
    tip: &str,
    date: &str,
    defs_by_file: &std::collections::HashMap<u32, std::collections::HashSet<u32>>,
) -> Vec<Value> {
    let root_path = std::path::Path::new(root);
    let mut hits: Vec<Value> = ingest
        .files
        .iter()
        .flat_map(|fi| {
            let rel_path = interner.resolve(&fi.path);
            scan_file_whereis_hits(
                root_path,
                rel_path,
                symbol_name,
                tip,
                date,
                defs_by_file.get(&fi.id),
            )
        })
        .collect();
    if hits.is_empty() {
        let mut h = collect_head_defs(ingest, interner, root, symbol_name);
        let mut refs = collect_head_refs(ingest, interner, root, symbol_name);
        h.append(&mut refs);
        for item in &mut h {
            item["tip"] = json!(tip);
            item["date"] = json!(date);
        }
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
    let (tip, date) = head_tip_and_date(root, &at_stamp);
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

    let mut hits = collect_worktree_whereis_hits(
        ingest,
        interner,
        root,
        symbol_name,
        &tip,
        &date,
        &defs_by_file,
    );

    let on_head = !hits.is_empty();
    let blobs = ingest.files.len();
    let (refs_scanned, mut branch_hits) = scan_local_branch_hits(root, symbol_name, on_head);
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
