use crate::domain::{IngestResult, Interner};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;

/// Execute `mentions` tool: find documentation naming a symbol in backticks.
pub fn call_mentions(ingest: &IngestResult, interner: &Interner, symbol_name: &str) -> Value {
    call_mentions_in_root(".", ingest, interner, symbol_name)
}

pub fn call_mentions_in_root(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    symbol_name: &str,
) -> Value {
    let mut matching_docs = Vec::new();
    let backtick_target = format!("`{symbol_name}`");
    let root_path = std::path::Path::new(root);

    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        if (path.ends_with(".md") || path.contains("doc"))
            && let Ok(content) =
                fs::read_to_string(root_path.join(path)).or_else(|_| fs::read_to_string(path))
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
    let clean = word.trim_matches(|c: char| matches!(c, '`' | '(' | ')' | '[' | ']' | ',' | ';'));
    let (target_file, line_str) = clean.split_once(':')?;
    let target_line = line_str
        .trim_matches(|c: char| !c.is_ascii_digit())
        .parse::<u32>()
        .ok()?;
    let &total_lines = file_map.get(target_file)?;
    if target_line > total_lines {
        Some(json!({
            "doc": doc_path,
            "line": doc_line,
            "doc_line": doc_line,
            "referenced": format!("{target_file}:{target_line}"),
            "target_file": target_file,
            "claimed_line": target_line,
            "actual_lines": total_lines,
            "why": "line-out-of-bounds"
        }))
    } else {
        None
    }
}

fn is_code_shaped_ident(sym: &str) -> bool {
    let bytes = sym.as_bytes();
    let has_underscore = bytes.contains(&b'_');
    let camel_seam = bytes
        .windows(2)
        .any(|w| w[0].is_ascii_lowercase() && w[1].is_ascii_uppercase());
    let k_const = bytes.len() > 1 && bytes[0] == b'k' && bytes[1].is_ascii_uppercase();
    has_underscore || camel_seam || k_const
}

fn is_language_builtin_ident(sym: &str) -> bool {
    matches!(
        sym,
        "AbortController"
            | "AggregateError"
            | "ArrayBuffer"
            | "AsyncFunction"
            | "BigInt64Array"
            | "BigUint64Array"
            | "DataView"
            | "EvalError"
            | "FinalizationRegistry"
            | "Float32Array"
            | "Float64Array"
            | "Int16Array"
            | "Int32Array"
            | "Int8Array"
            | "RangeError"
            | "ReferenceError"
            | "SharedArrayBuffer"
            | "SyntaxError"
            | "TextDecoder"
            | "TextEncoder"
            | "TypeError"
            | "URIError"
            | "URLSearchParams"
            | "Uint16Array"
            | "Uint32Array"
            | "Uint8Array"
            | "Uint8ClampedArray"
            | "WeakMap"
            | "WeakRef"
            | "WeakSet"
            | "clearInterval"
            | "clearTimeout"
            | "decodeURIComponent"
            | "encodeURIComponent"
            | "globalThis"
            | "queueMicrotask"
            | "setInterval"
            | "setTimeout"
            | "structuredClone"
            | "ValueError"
            | "RuntimeError"
            | "KeyError"
            | "IndexError"
            | "OSError"
            | "AttributeError"
            | "NotImplementedError"
            | "FileNotFoundError"
            | "PermissionError"
            | "TimeoutError"
            | "ConnectionError"
            | "ImportError"
            | "AssertionError"
    )
}

fn is_candidate_symbol_anchor(span: &str) -> Option<&str> {
    let trimmed = span.trim();
    let had_parens = trimmed.ends_with("()");
    let raw = trimmed.strip_suffix("()").unwrap_or(trimmed);
    if raw.contains('/')
        || raw.contains(' ')
        || raw.contains('=')
        || raw.contains('-')
        || raw.starts_with('@')
    {
        return None;
    }
    let sym = raw
        .rsplit("::")
        .next()
        .unwrap_or(raw)
        .rsplit('.')
        .next()
        .unwrap_or(raw);
    let valid_ident = sym.len() >= 4
        && !is_language_builtin_ident(sym)
        && sym
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && sym.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if valid_ident && (had_parens || is_code_shaped_ident(sym)) {
        Some(sym)
    } else {
        None
    }
}

fn collect_line_backtick_candidates<'a>(
    line: &'a str,
    doc_line: usize,
    sym_set: &std::collections::HashSet<&str>,
    resolving_lines: &mut Vec<usize>,
    pending_missing: &mut Vec<(usize, &'a str, bool)>,
) {
    if sym_set.is_empty() {
        return;
    }
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        let after_open = &rest[start + 1..];
        let Some(end) = after_open.find('`') else {
            break;
        };
        let span = &after_open[..end];
        if let Some(sym) = is_candidate_symbol_anchor(span) {
            if sym_set.contains(sym) {
                if resolving_lines.last().copied() != Some(doc_line) {
                    resolving_lines.push(doc_line);
                }
            } else {
                pending_missing.push((doc_line, sym, span.trim().ends_with("()")));
            }
        }
        rest = &after_open[end + 1..];
    }
}

/// Execute `doc_drift` tool: verify checkable doc references.
pub fn call_doc_drift(ingest: &IngestResult, interner: &Interner) -> Value {
    call_doc_drift_in_root(".", ingest, interner)
}

fn flush_corroborated_missing(
    doc_path: &str,
    resolving_lines: &[usize],
    pending_missing: Vec<(usize, &str, bool)>,
    drift_items: &mut Vec<Value>,
) {
    for (doc_line, sym, had_parens) in pending_missing {
        if had_parens || resolving_lines.iter().any(|&r| r.abs_diff(doc_line) <= 5) {
            drift_items.push(json!({
                "doc": doc_path,
                "line": doc_line,
                "doc_line": doc_line,
                "kind": "symbol",
                "referenced": sym,
                "why": "undefined"
            }));
        }
    }
}

fn scan_doc_text_for_drift(
    text: &str,
    doc_path: &str,
    file_map: &HashMap<&str, u32>,
    sym_set: &std::collections::HashSet<&str>,
    drift_items: &mut Vec<Value>,
) {
    let mut in_code_fence = false;
    let mut resolving_lines = Vec::new();
    let mut pending_missing = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }
        let doc_line = line_idx + 1;
        for word in line.split_whitespace() {
            if let Some(item) = check_word_drift(word, doc_path, doc_line, file_map) {
                drift_items.push(item);
            }
        }
        collect_line_backtick_candidates(
            line,
            doc_line,
            sym_set,
            &mut resolving_lines,
            &mut pending_missing,
        );
    }
    flush_corroborated_missing(doc_path, &resolving_lines, pending_missing, drift_items);
}

pub fn call_doc_drift_in_root(root: &str, ingest: &IngestResult, interner: &Interner) -> Value {
    call_doc_drift_with_history(root, ingest, interner, false)
}

fn enrich_drift_item_history(root: &str, item: &mut Value, head_sha: &str) {
    let Some(sym) = item["referenced"].as_str().map(str::to_string) else {
        return;
    };
    let Some(hist) = probe_name_history(root, &sym, head_sha) else {
        return;
    };
    let fate = &hist["fate"];
    if let Some(v) = fate["v"].as_str() {
        item["fate"] = Value::String(v.to_string());
    }
    if let Some(c) = fate["commit"].as_str() {
        item["commit"] = Value::String(c.to_string());
    }
    if let Some(d) = fate["date"].as_str() {
        item["date"] = Value::String(d.to_string());
    }
    if let Some(s) = fate["subject"].as_str() {
        item["subject"] = Value::String(s.to_string());
    }
}

fn build_known_code_ident_set<'a>(
    ingest: &'a IngestResult,
    interner: &'a Interner,
) -> std::collections::HashSet<&'a str> {
    let mut sym_set: std::collections::HashSet<&str> = ingest
        .symbols
        .iter()
        .filter(|s| s.kind != crate::domain::SymKind::Section)
        .map(|s| interner.resolve(&s.name))
        .collect();
    for sc in ingest.symbols.iter().filter_map(|s| s.scope.as_ref()) {
        sym_set.insert(interner.resolve(sc));
    }
    for r in ingest
        .references
        .iter()
        .filter(|r| !r.lang.semantics().is_prose_document())
    {
        sym_set.insert(interner.resolve(&r.callee_name));
        if let Some(q) = &r.qualifier {
            sym_set.insert(interner.resolve(q));
        }
    }
    sym_set
}

fn collect_file_wanted_tokens(
    bytes: &[u8],
    wants_first: &[bool; 256],
    wanted: &std::collections::HashSet<String>,
) -> Vec<String> {
    let mut found = Vec::new();
    for tok in bytes.split(|&b| !b.is_ascii_alphanumeric() && b != b'_') {
        if tok.len() >= 4
            && wants_first[tok[0] as usize]
            && let Ok(s) = std::str::from_utf8(tok)
            && wanted.contains(s)
        {
            found.push(s.to_string());
        }
    }
    found
}

fn extend_from_scm_queries(
    root_path: &std::path::Path,
    wants_first: &[bool; 256],
    wanted: &std::collections::HashSet<String>,
    found_in_code: &mut std::collections::HashSet<String>,
) {
    let Ok(entries) = fs::read_dir(root_path.join("queries")) else {
        return;
    };
    for entry in entries.flatten() {
        let scm_path = entry.path().join("tags.scm");
        if let Ok(bytes) = fs::read(&scm_path) {
            found_in_code.extend(collect_file_wanted_tokens(&bytes, wants_first, wanted));
        }
    }
}

fn filter_present_in_code_drift(
    root_path: &std::path::Path,
    ingest: &IngestResult,
    interner: &Interner,
    drift_items: &mut Vec<Value>,
) {
    use rayon::prelude::*;
    let wanted: std::collections::HashSet<String> = drift_items
        .iter()
        .filter(|d| d["kind"].as_str() == Some("symbol"))
        .filter_map(|d| d["referenced"].as_str().map(str::to_string))
        .collect();
    if wanted.is_empty() {
        return;
    }
    let mut wants_first = [false; 256];
    for w in &wanted {
        if let Some(&b) = w.as_bytes().first() {
            wants_first[b as usize] = true;
        }
    }
    let mut found_in_code: std::collections::HashSet<String> = ingest
        .files
        .par_iter()
        .filter(|f| !f.lang.semantics().is_prose_document())
        .flat_map_iter(|f| {
            let rel = interner.resolve(&f.path);
            match fs::read(root_path.join(rel)).or_else(|_| fs::read(rel)) {
                Ok(bytes) => collect_file_wanted_tokens(&bytes, &wants_first, &wanted),
                Err(_) => Vec::new(),
            }
        })
        .collect();
    extend_from_scm_queries(root_path, &wants_first, &wanted, &mut found_in_code);
    drift_items.retain(|d| {
        if d["kind"].as_str() == Some("symbol")
            && let Some(sym) = d["referenced"].as_str()
        {
            !found_in_code.contains(sym)
        } else {
            true
        }
    });
}

pub fn call_doc_drift_with_history(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    with_history: bool,
) -> Value {
    let mut drift_items = Vec::new();
    let file_map: HashMap<&str, u32> = ingest
        .files
        .iter()
        .map(|f| (interner.resolve(&f.path), f.line_count))
        .collect();
    let sym_set = build_known_code_ident_set(ingest, interner);
    let root_path = std::path::Path::new(root);

    for file in &ingest.files {
        let doc_path = interner.resolve(&file.path);
        if doc_path.ends_with(".md")
            && let Ok(text) = fs::read_to_string(root_path.join(doc_path))
                .or_else(|_| fs::read_to_string(doc_path))
        {
            scan_doc_text_for_drift(&text, doc_path, &file_map, &sym_set, &mut drift_items);
        }
    }
    filter_present_in_code_drift(root_path, ingest, interner, &mut drift_items);
    if with_history {
        let head_sha = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
        for item in &mut drift_items {
            enrich_drift_item_history(root, item, &head_sha);
        }
    }

    json!({
        "root": root,
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
    root_path: &std::path::Path,
    path: &str,
    flags: &mut Vec<Value>,
    compile_count: &mut usize,
    cmake_count: &mut usize,
    env_count: &mut usize,
) {
    let Ok(text) = fs::read_to_string(root_path.join(path)).or_else(|_| fs::read_to_string(path))
    else {
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
    call_flags_in_root(".", ingest, interner)
}

pub fn call_flags_in_root(root: &str, ingest: &IngestResult, interner: &Interner) -> Value {
    let root_path = std::path::Path::new(root);
    let mut flags = Vec::new();
    let mut compile_count = 0usize;
    let mut cmake_count = 0usize;
    let mut env_count = 0usize;

    for file in &ingest.files {
        if !file.lang.semantics().is_prose_document() {
            let path = interner.resolve(&file.path);
            scan_file_flags(
                root_path,
                path,
                &mut flags,
                &mut compile_count,
                &mut cmake_count,
                &mut env_count,
            );
        }
    }

    let dark_gates = compile_count + cmake_count;
    json!({
        "root": root,
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

fn classify_code_line(code: &str) -> &'static str {
    const DEF_PREFIXES: [&str; 5] = ["fn ", "pub fn ", "def ", "class ", "struct "];
    if DEF_PREFIXES.iter().any(|&p| code.starts_with(p)) {
        "def"
    } else {
        "ref"
    }
}

fn parse_branch_grep_line(line: &str, branch: &str, symbol_name: &str) -> Option<Value> {
    let mut parts = line.splitn(3, ':');
    let _ref_prefix = parts.next()?;
    let path = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("");
    let mut line_parts = rest.splitn(2, ':');
    let line_num: u64 = line_parts.next().and_then(|l| l.parse().ok()).unwrap_or(0);
    let code_text = line_parts.next().unwrap_or("").trim();
    let kind = classify_code_line(code_text);
    Some(json!({
        "ref": branch,
        "symbol": symbol_name,
        "kind": kind,
        "file": path,
        "path": path,
        "line": line_num,
        "text": code_text,
        "on_head": false
    }))
}

fn should_scan_branch(branch: &str, on_head: bool) -> bool {
    !branch.is_empty() && branch != "HEAD" && !(branch == "main" && on_head)
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
    for raw_branch in branch_output.lines() {
        let branch = raw_branch.trim();
        if !should_scan_branch(branch, on_head) {
            continue;
        }
        refs_scanned += 1;
        let Ok(grep_out) = std::process::Command::new("git")
            .args([
                "-C",
                root,
                "grep",
                "--threads=1",
                "-n",
                "-F",
                symbol_name,
                branch,
            ])
            .output()
        else {
            continue;
        };
        if !grep_out.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&grep_out.stdout);
        for line in text.lines().take(60) {
            if let Some(hit) = parse_branch_grep_line(line, branch, symbol_name) {
                hits.push(hit);
            }
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
        assert!(whereis_res["count"].as_u64().unwrap_or(0) >= 1);
        let whereis_hist = call_whereis(&ingest, &interner, ".", "nonexistent_sym", true);
        assert!(whereis_hist.get("history").is_some());
        let stray_res = call_stray_content(".");
        assert!(stray_res.get("head").is_some());
    }

    #[test]
    fn test_branch_grep_parsing() {
        assert_eq!(classify_code_line("fn foo()"), "def");
        assert_eq!(classify_code_line("pub fn bar()"), "def");
        assert_eq!(classify_code_line("let x = foo();"), "ref");
        assert!(should_scan_branch("feature-1", true));
        assert!(!should_scan_branch("HEAD", true));
        assert!(!should_scan_branch("", true));
        assert!(!should_scan_branch("main", true));
        assert!(should_scan_branch("main", false));

        let hit = parse_branch_grep_line(
            "origin/main:src/lib.rs:42:fn hello()",
            "origin/main",
            "hello",
        );
        assert!(hit.is_some());
        let val = hit.unwrap();
        assert_eq!(val["kind"], "def");
        assert_eq!(val["line"], 42);
        assert_eq!(val["file"], "src/lib.rs");
    }

    #[test]
    fn test_doc_drift_backtick_and_line_anchors() {
        let mut file_map = HashMap::new();
        file_map.insert("src/lib.rs", 20);
        let mut sym_set = std::collections::HashSet::new();
        sym_set.insert("existingFn");

        let mut drift = Vec::new();
        scan_doc_text_for_drift(
            "See `existingFn` and `deletedFunction` at src/lib.rs:50\n```rust\n`ignoredInFence`\n```",
            "README.md",
            &file_map,
            &sym_set,
            &mut drift,
        );
        assert_eq!(drift.len(), 2);
        assert!(drift.iter().any(|d| d["referenced"] == "src/lib.rs:50"));
        assert!(drift.iter().any(|d| d["referenced"] == "deletedFunction"));
        let mut ingest = IngestResult::new();
        let interner = Interner::new();
        ingest.files.push(crate::domain::FileInfo {
            id: 0,
            path: interner.intern("src/lib.rs"),
            lang: crate::domain::Lang::Rust,
            line_count: 20,
            byte_size: 100,
        });
        filter_present_in_code_drift(std::path::Path::new("."), &ingest, &interner, &mut drift);
        assert_eq!(drift.len(), 2);
        let mut wanted = std::collections::HashSet::new();
        wanted.insert("deletedFunction".to_string());
        let mut wants_first = [false; 256];
        wants_first[b'd' as usize] = true;
        let found = collect_file_wanted_tokens(
            b"fn helper() { deletedFunction(); }",
            &wants_first,
            &wanted,
        );
        assert_eq!(found, vec!["deletedFunction".to_string()]);
        enrich_drift_item_history(".", &mut drift[1], "HEAD");
        assert!(drift[1].get("fate").is_some());
        let mut xml_buf = Vec::new();
        crate::serialize::serialize_doc_drift_xml(&json!({"drift": drift}), &mut xml_buf).unwrap();
        let xml_str = String::from_utf8(xml_buf).unwrap();
        assert!(xml_str.contains("fate=\""));
    }
}
