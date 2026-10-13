use crate::domain::{IngestResult, Interner};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;

/// Execute `mentions` tool: find documentation naming a symbol in backticks.
pub fn call_mentions(ingest: &IngestResult, interner: &Interner, symbol_name: &str) -> Value {
    let mut matching_docs = Vec::new();
    let backtick_target = format!("`{symbol_name}`");

    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        if (path.ends_with(".md") || path.contains("doc"))
            && let Ok(content) = fs::read_to_string(path)
        {
            let occurrences = content.matches(&backtick_target).count();
            if occurrences > 0 {
                matching_docs.push(json!({
                    "file": path,
                    "mentions": occurrences
                }));
            }
        }
    }

    json!({
        "symbol": symbol_name,
        "docs": matching_docs,
        "total_mentions": matching_docs.iter().map(|d| d["mentions"].as_u64().unwrap_or(0)).sum::<u64>()
    })
}

/// Execute `doc_drift` tool: verify checkable doc references.
fn check_word_drift(
    word: &str,
    doc_path: &str,
    doc_line: usize,
    file_map: &HashMap<&str, u32>,
) -> Option<Value> {
    let (target_file, line_str) = word.split_once(':')?;
    let target_line = line_str
        .trim_matches(|c: char| !c.is_ascii_digit())
        .parse::<u32>()
        .ok()?;
    let &total_lines = file_map.get(target_file)?;
    if target_line > total_lines {
        Some(json!({
            "doc": doc_path,
            "doc_line": doc_line,
            "target_file": target_file,
            "claimed_line": target_line,
            "actual_lines": total_lines,
            "why": "line-out-of-bounds"
        }))
    } else {
        None
    }
}

/// Execute `doc_drift` tool: verify checkable doc references.
pub fn call_doc_drift(ingest: &IngestResult, interner: &Interner) -> Value {
    let mut drift_items = Vec::new();

    let file_map: HashMap<&str, u32> = ingest
        .files
        .iter()
        .map(|f| (interner.resolve(&f.path), f.line_count))
        .collect();

    for file in &ingest.files {
        let doc_path = interner.resolve(&file.path);
        if !doc_path.ends_with(".md") {
            continue;
        }

        let Ok(text) = fs::read_to_string(doc_path) else {
            continue;
        };
        for (line_idx, line) in text.lines().enumerate() {
            for word in line.split_whitespace() {
                if let Some(item) = check_word_drift(word, doc_path, line_idx + 1, &file_map) {
                    drift_items.push(item);
                }
            }
        }
    }

    json!({
        "drift_count": drift_items.len(),
        "drift": drift_items
    })
}

fn extract_env_identifier(line: &str) -> Option<String> {
    for pattern in &[
        "env::var(\"",
        "getenv(\"",
        "os.getenv(\"",
        "os.environ.get(\"",
        "std::getenv(\"",
    ] {
        if let Some(pos) = line.find(pattern) {
            let rest = &line[pos + pattern.len()..];
            if let Some(end) = rest.find('"') {
                let name = &rest[..end];
                if !name.is_empty() && name.len() <= 128 {
                    return Some(name.to_string());
                }
            }
        }
    }
    if let Some(pos) = line.find("process.env.") {
        let rest = &line[pos + "process.env.".len()..];
        let ident: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !ident.is_empty() {
            return Some(ident);
        }
    }
    None
}

fn extract_compile_guard(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("#ifndef ") {
        let name: String = rest
            .split_whitespace()
            .next()?
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            return Some(name);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("#ifdef ") {
        let name: String = rest
            .split_whitespace()
            .next()?
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}

fn extract_cmake_option(line: &str, path: &str) -> Option<String> {
    if !path.ends_with(".cmake") && !path.ends_with("CMakeLists.txt") {
        return None;
    }
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("option(") {
        let name: String = rest
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}

fn scan_file_flags(
    path: &str,
    flags: &mut Vec<Value>,
    compile_count: &mut usize,
    cmake_count: &mut usize,
    env_count: &mut usize,
) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    for (idx, line) in text.lines().enumerate() {
        if let Some(guard) = extract_compile_guard(line) {
            *compile_count += 1;
            flags.push(json!({
                "name": guard,
                "flag": guard,
                "kind": "compile",
                "file": path,
                "path": path,
                "line": idx + 1
            }));
        } else if let Some(opt) = extract_cmake_option(line, path) {
            *cmake_count += 1;
            flags.push(json!({
                "name": opt,
                "flag": opt,
                "kind": "cmake",
                "file": path,
                "path": path,
                "line": idx + 1
            }));
        } else if let Some(env_var) = extract_env_identifier(line) {
            *env_count += 1;
            flags.push(json!({
                "name": env_var,
                "flag": env_var,
                "kind": "env",
                "file": path,
                "path": path,
                "line": idx + 1
            }));
        }
    }
}

/// Execute `flags` tool: detect build flags and runtime configuration gates.
pub fn call_flags(ingest: &IngestResult, interner: &Interner) -> Value {
    let mut flags = Vec::new();
    let mut compile_count = 0usize;
    let mut cmake_count = 0usize;
    let mut env_count = 0usize;

    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        scan_file_flags(
            path,
            &mut flags,
            &mut compile_count,
            &mut cmake_count,
            &mut env_count,
        );
    }

    let dark_gates = env_count;
    json!({
        "files": ingest.files.len(),
        "flags_count": flags.len(),
        "compile_count": compile_count,
        "cmake_count": cmake_count,
        "env_count": env_count,
        "dark_gates": dark_gates,
        "gates": flags.clone(),
        "flags": flags
    })
}

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

fn scan_local_branch_hits(root: &str, symbol_name: &str, on_head: bool) -> (usize, Vec<Value>) {
    let mut hits = Vec::new();
    let mut refs_scanned = 0usize;
    let Ok(out) = std::process::Command::new("git")
        .args([
            "-C",
            root,
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
        ])
        .output()
    else {
        return (0, hits);
    };
    if !out.status.success() {
        return (0, hits);
    }
    let branch_output = String::from_utf8_lossy(&out.stdout);
    for branch in branch_output.lines() {
        let branch = branch.trim();
        if branch.is_empty() || branch == "HEAD" || (branch == "main" && on_head) {
            continue;
        }
        refs_scanned += 1;
        let Ok(grep_out) = std::process::Command::new("git")
            .args(["-C", root, "grep", "-n", "-F", symbol_name, branch])
            .output()
        else {
            continue;
        };
        if !grep_out.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&grep_out.stdout);
        for line in text.lines().take(60) {
            let mut parts = line.splitn(3, ':');
            let _ref_prefix = parts.next();
            let path = parts.next().unwrap_or("");
            let rest = parts.next().unwrap_or("");
            let mut line_parts = rest.splitn(2, ':');
            let line_num: u64 = line_parts.next().and_then(|l| l.parse().ok()).unwrap_or(0);
            let code_text = line_parts.next().unwrap_or("").trim();
            let kind = if code_text.starts_with("fn ")
                || code_text.starts_with("pub fn ")
                || code_text.starts_with("def ")
                || code_text.starts_with("class ")
                || code_text.starts_with("struct ")
            {
                "def"
            } else {
                "ref"
            };
            hits.push(json!({
                "ref": branch,
                "symbol": symbol_name,
                "kind": kind,
                "file": path,
                "path": path,
                "line": line_num,
                "text": code_text,
                "on_head": false
            }));
        }
    }
    (refs_scanned, hits)
}

fn collect_head_defs(
    ingest: &IngestResult,
    interner: &Interner,
    root: &str,
    sym_name: &str,
) -> Vec<Value> {
    let mut hits = Vec::new();
    for sym in &ingest.symbols {
        let n = interner.resolve(&sym.name);
        if n == sym_name {
            let file_path = ingest
                .files
                .get(sym.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            let full_path = if root == "." {
                file_path.to_string()
            } else {
                format!("{root}/{file_path}")
            };
            let text = get_line_snippet(&full_path, sym.line, &format!("{} {}", sym.kind.tag(), n));
            hits.push(json!({
                "ref": "HEAD",
                "symbol": n,
                "kind": "def",
                "file": file_path,
                "path": file_path,
                "line": sym.line,
                "text": text,
                "on_head": true
            }));
        }
    }
    hits
}

fn collect_head_refs(
    ingest: &IngestResult,
    interner: &Interner,
    root: &str,
    sym_name: &str,
) -> Vec<Value> {
    let mut hits = Vec::new();
    for r in &ingest.references {
        let n = interner.resolve(&r.callee_name);
        if n == sym_name {
            let file_path = ingest
                .files
                .get(r.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            let full_path = if root == "." {
                file_path.to_string()
            } else {
                format!("{root}/{file_path}")
            };
            let text = get_line_snippet(&full_path, r.line, n);
            hits.push(json!({
                "ref": "HEAD",
                "symbol": n,
                "kind": "ref",
                "file": file_path,
                "path": file_path,
                "line": r.line,
                "text": text,
                "on_head": true
            }));
        }
    }
    hits
}

fn probe_name_history(root: &str, sym_name: &str, head_sha: &str) -> Option<Value> {
    let out = std::process::Command::new("git")
        .args([
            "-C",
            root,
            "log",
            "-S",
            sym_name,
            "-1",
            "--format=%h%x09%ad%x09%s",
            "--date=short",
        ])
        .output()
        .ok()?;
    if out.status.success() {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !text.is_empty() {
            let mut parts = text.splitn(3, '\t');
            let sha = parts.next().unwrap_or("").to_string();
            let date = parts.next().unwrap_or("").to_string();
            let subject = parts.next().unwrap_or("").to_string();
            return Some(json!({
                "probed": 1,
                "head": head_sha,
                "fate": {
                    "v": "removed",
                    "commit": sha,
                    "date": date,
                    "subject": subject
                }
            }));
        }
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

/// Execute `whereis` tool: identify where a symbol lives (definitions and references).
pub fn call_whereis(
    ingest: &IngestResult,
    interner: &Interner,
    root: &str,
    symbol_name: &str,
    with_history: bool,
) -> Value {
    let at_stamp = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
    let mut hits = collect_head_defs(ingest, interner, root, symbol_name);
    let mut refs = collect_head_refs(ingest, interner, root, symbol_name);
    hits.append(&mut refs);

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

fn count_rev_list(root: &str, base: &str, branch: &str) -> u64 {
    std::process::Command::new("git")
        .args([
            "-C",
            root,
            "rev-list",
            "--count",
            &format!("{base}..{branch}"),
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(0)
}

fn inspect_branch_stray(root: &str, branch: &str, head_ref: &str) -> Option<(String, Value)> {
    if branch.is_empty() || branch == "HEAD" || branch == head_ref {
        return None;
    }
    let mb = std::process::Command::new("git")
        .args(["-C", root, "merge-base", "HEAD", branch])
        .output()
        .ok()?;
    if !mb.status.success() {
        return Some(("unknown".to_string(), json!({})));
    }
    let base = String::from_utf8_lossy(&mb.stdout).trim().to_string();
    let short_base = base[..9.min(base.len())].to_string();
    let unmerged = count_rev_list(root, &short_base, branch);
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

fn gather_branch_stray(root: &str, head_ref: &str) -> (usize, usize, usize, usize, Vec<Value>) {
    let mut refs_count = 0usize;
    let mut unmerged = 0usize;
    let mut merged = 0usize;
    let mut unknown = 0usize;
    let mut branches = Vec::new();

    let Ok(out) = std::process::Command::new("git")
        .args([
            "-C",
            root,
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads/",
        ])
        .output()
    else {
        return (0, 0, 0, 0, Vec::new());
    };
    if !out.status.success() {
        return (0, 0, 0, 0, Vec::new());
    }

    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let branch = line.trim();
        if let Some((verdict, branch_val)) = inspect_branch_stray(root, branch, head_ref) {
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
    let head_sha = std::process::Command::new("git")
        .args(["-C", root, "rev-parse", "--short=9", "HEAD"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let head_ref = std::process::Command::new("git")
        .args(["-C", root, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "HEAD".to_string());

    let (refs_count, unmerged, merged, unknown, branches) = gather_branch_stray(root, &head_ref);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mentions_empty() {
        let ingest = IngestResult::new();
        let interner = Interner::new();
        let res = call_mentions(&ingest, &interner, "nonexistent");
        assert_eq!(res["total_mentions"], 0);
    }

    #[test]
    fn test_owners_attribution() {
        let commits = vec![
            crate::git::history::CommitInfo {
                id: "c1".to_string(),
                timestamp: 1000,
                author: "alice@example.com".to_string(),
                changed_files: vec!["src/lib.rs".to_string()],
            },
            crate::git::history::CommitInfo {
                id: "c2".to_string(),
                timestamp: 2000,
                author: "bob@example.com".to_string(),
                changed_files: vec!["src/lib.rs".to_string()],
            },
        ];
        let res = call_owners(Some(&commits), None, None, None);
        assert_eq!(res["files_analyzed"], 1);
        let files = res["files"].as_array().unwrap();
        assert_eq!(files[0]["authors"], 2);
    }

    #[test]
    fn test_gate_extractors() {
        assert_eq!(
            extract_env_identifier("std::env::var(\"FOO\")"),
            Some("FOO".into())
        );
        assert_eq!(
            extract_env_identifier("getenv(\"BAR\")"),
            Some("BAR".into())
        );
        assert_eq!(
            extract_env_identifier("os.getenv(\"BAZ\")"),
            Some("BAZ".into())
        );
        assert_eq!(
            extract_env_identifier("os.environ.get(\"QUX\")"),
            Some("QUX".into())
        );
        assert_eq!(
            extract_env_identifier("process.env.VAR_123"),
            Some("VAR_123".into())
        );
        assert_eq!(extract_env_identifier("let x = 42;"), None);

        assert_eq!(
            extract_compile_guard("#ifndef MY_FLAG"),
            Some("MY_FLAG".into())
        );
        assert_eq!(
            extract_compile_guard("#ifdef OTHER_FLAG"),
            Some("OTHER_FLAG".into())
        );
        assert_eq!(extract_compile_guard("#include <stdio.h>"), None);

        assert_eq!(
            extract_cmake_option("option(ENABLE_TESTS \"doc\" ON)", "CMakeLists.txt"),
            Some("ENABLE_TESTS".into())
        );
        assert_eq!(
            extract_cmake_option("option(ENABLE_TESTS \"doc\" ON)", "main.rs"),
            None
        );
    }

    #[test]
    fn test_branch_stray_inspection() {
        assert_eq!(inspect_branch_stray(".", "", "main"), None);
        assert_eq!(inspect_branch_stray(".", "HEAD", "main"), None);
        assert_eq!(inspect_branch_stray(".", "main", "main"), None);
        let res = inspect_branch_stray(".", "nonexistent-branch-xyz", "main");
        assert!(res.is_some());
        assert_eq!(count_rev_list(".", "HEAD", "HEAD"), 0);
    }

    #[test]
    fn test_whereis_and_stray() {
        let mut ingest = IngestResult::new();
        let interner = Interner::new();
        let f0 = crate::domain::FileInfo {
            id: 0,
            path: interner.intern("src/main.rs"),
            lang: crate::domain::Lang::Rust,
            line_count: 10,
            byte_size: 100,
        };
        let sym = crate::domain::Symbol::new(
            0,
            interner.intern("main"),
            interner.intern("c_main"),
            crate::domain::SymKind::Function,
            crate::domain::Lang::Rust,
            0,
            1,
        );
        ingest.files.push(f0);
        ingest.symbols.push(sym);

        let whereis_res = call_whereis(&ingest, &interner, ".", "main", false);
        assert_eq!(whereis_res["count"], 1);
        let whereis_hist = call_whereis(&ingest, &interner, ".", "nonexistent_sym", true);
        assert!(whereis_hist.get("history").is_some());
        let stray_res = call_stray_content(".");
        assert!(stray_res.get("head").is_some());
    }
}
