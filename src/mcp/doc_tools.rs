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
            && let Ok(content) = crate::domain::read_rel_string(root_path, path)
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
fn extract_line_resolved_symbols<'a>(
    line: &'a str,
    sym_set: &std::collections::HashSet<&str>,
) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        let after_open = &rest[start + 1..];
        let Some(end) = after_open.find('`') else {
            break;
        };
        if let Some(sym) = is_candidate_symbol_anchor(&after_open[..end])
            && sym_set.contains(sym)
        {
            out.push(sym);
        }
        rest = &after_open[end + 1..];
    }
    out
}

fn check_line_moved_drift(
    doc_path: &str,
    doc_line: usize,
    target_file: &str,
    target_line: u32,
    line_syms: &[&str],
    ingest: &IngestResult,
    interner: &Interner,
) -> Option<Value> {
    use crate::domain::CorpusView;
    if line_syms.is_empty() || ingest.symbols.is_empty() {
        return None;
    }
    let view = (ingest, interner);
    let got_sym = crate::navigate::resolve_symbol_at(target_file, target_line, ingest, interner);
    for &sym_name in line_syms {
        if got_sym.is_some_and(|gs| view.sym_name(gs) == sym_name) {
            return None;
        }
        let Some(moved) = ingest
            .symbols
            .iter()
            .find(|s| view.sym_name(s) == sym_name && view.sym_path(s) == target_file)
        else {
            continue;
        };
        if target_line < moved.line || target_line > moved.end_line() {
            let got_name = got_sym.map_or("", |gs| view.sym_name(gs));
            return Some(json!({
                "doc": doc_path,
                "line": doc_line,
                "doc_line": doc_line,
                "kind": "file-line",
                "referenced": format!("{target_file}:{target_line}"),
                "target_file": target_file,
                "claimed_line": target_line,
                "sym": sym_name,
                "got": got_name,
                "tgt": format!("{target_file}:{}", moved.line),
                "why": "line-moved"
            }));
        }
    }
    None
}

fn check_word_drift(
    word: &str,
    doc_path: &str,
    doc_line: usize,
    file_map: &HashMap<&str, u32>,
    line_syms: &[&str],
    ingest: &IngestResult,
    interner: &Interner,
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
            "kind": "file-line",
            "referenced": format!("{target_file}:{target_line}"),
            "target_file": target_file,
            "claimed_line": target_line,
            "actual_lines": total_lines,
            "why": "line-out-of-bounds"
        }))
    } else {
        check_line_moved_drift(
            doc_path,
            doc_line,
            target_file,
            target_line,
            line_syms,
            ingest,
            interner,
        )
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

#[derive(Default, Clone, Copy)]
pub struct DocAnchorCounts {
    pub total: usize,
    pub checked: usize,
}

fn is_counted_backtick_anchor(span: &str) -> bool {
    let t = span.trim();
    !t.is_empty()
        && !t.starts_with('-')
        && !t.starts_with('@')
        && !t
            .bytes()
            .any(|b| matches!(b, b' ' | b'\t' | b'<' | b'>' | b'='))
        && t.bytes().any(|b| b.is_ascii_alphabetic())
}

fn record_backtick_span<'a>(
    span: &'a str,
    doc_line: usize,
    sym_set: &std::collections::HashSet<&str>,
    counts: &mut DocAnchorCounts,
    resolving_lines: &mut Vec<usize>,
    pending_missing: &mut Vec<(usize, &'a str, bool)>,
) {
    if !is_counted_backtick_anchor(span) {
        return;
    }
    counts.total += 1;
    let Some(sym) = is_candidate_symbol_anchor(span) else {
        return;
    };
    counts.checked += 1;
    if !sym_set.contains(sym) {
        pending_missing.push((doc_line, sym, span.trim().ends_with("()")));
    } else if resolving_lines.last().copied() != Some(doc_line) {
        resolving_lines.push(doc_line);
    }
}

fn collect_line_backtick_candidates<'a>(
    line: &'a str,
    doc_line: usize,
    sym_set: &std::collections::HashSet<&str>,
    resolving_lines: &mut Vec<usize>,
    pending_missing: &mut Vec<(usize, &'a str, bool)>,
) -> DocAnchorCounts {
    let mut counts = DocAnchorCounts::default();
    if sym_set.is_empty() {
        return counts;
    }
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        let after_open = &rest[start + 1..];
        let Some(end) = after_open.find('`') else {
            break;
        };
        record_backtick_span(
            &after_open[..end],
            doc_line,
            sym_set,
            &mut counts,
            resolving_lines,
            pending_missing,
        );
        rest = &after_open[end + 1..];
    }
    counts
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

fn is_dated_or_historical_doc(doc_path: &str, text: &str) -> bool {
    let lower = doc_path.to_ascii_lowercase();
    if lower.ends_with("changelog.md") || lower.contains("/changelog") {
        return true;
    }
    text.lines().take(15).any(|l| {
        let bytes = l.as_bytes();
        bytes.windows(10).any(|w| {
            w[0..4].iter().all(u8::is_ascii_digit)
                && w[4] == b'-'
                && w[5..7].iter().all(u8::is_ascii_digit)
                && w[7] == b'-'
                && w[8..10].iter().all(u8::is_ascii_digit)
        })
    })
}

pub fn scan_doc_text_for_drift(
    text: &str,
    doc_path: &str,
    file_map: &HashMap<&str, u32>,
    sym_set: &std::collections::HashSet<&str>,
    ingest: &IngestResult,
    interner: &Interner,
    drift_items: &mut Vec<Value>,
) -> DocAnchorCounts {
    let mut in_code_fence = false;
    let mut resolving_lines = Vec::new();
    let mut pending_missing = Vec::new();
    let mut counts = DocAnchorCounts::default();
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
        let line_syms = extract_line_resolved_symbols(line, sym_set);
        for word in line.split_whitespace() {
            if let Some(item) = check_word_drift(
                word, doc_path, doc_line, file_map, &line_syms, ingest, interner,
            ) {
                counts.total += 1;
                counts.checked += 1;
                drift_items.push(item);
            }
        }
        let line_counts = collect_line_backtick_candidates(
            line,
            doc_line,
            sym_set,
            &mut resolving_lines,
            &mut pending_missing,
        );
        counts.total += line_counts.total;
        counts.checked += line_counts.checked;
    }
    flush_corroborated_missing(doc_path, &resolving_lines, pending_missing, drift_items);
    counts
}

pub fn call_doc_drift_in_root(root: &str, ingest: &IngestResult, interner: &Interner) -> Value {
    call_doc_drift_with_history(root, ingest, interner, false)
}

pub fn enrich_drift_item_history(root: &str, item: &mut Value, head_sha: &str) {
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

pub fn collect_file_wanted_tokens(
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

pub fn filter_present_in_code_drift(
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
            crate::domain::read_rel_bytes(root_path, rel)
                .map(|bytes| collect_file_wanted_tokens(&bytes, &wants_first, &wanted))
                .unwrap_or_default()
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

#[derive(Default)]
struct CorpusDocSummary {
    docs_scanned: usize,
    dated_docs: usize,
    prose_docs: usize,
    anchors: usize,
    checked: usize,
}

fn scan_corpus_markdown_docs(
    root_path: &std::path::Path,
    ingest: &IngestResult,
    interner: &Interner,
    file_map: &HashMap<&str, u32>,
    sym_set: &std::collections::HashSet<&str>,
    drift_items: &mut Vec<Value>,
) -> CorpusDocSummary {
    let mut summary = CorpusDocSummary::default();
    for file in &ingest.files {
        let doc_path = interner.resolve(&file.path);
        if file.lang != crate::domain::Lang::Markdown && !doc_path.ends_with(".md") {
            continue;
        }
        let Ok(text) = crate::domain::read_rel_string(root_path, doc_path) else {
            continue;
        };
        summary.docs_scanned += 1;
        if is_dated_or_historical_doc(doc_path, &text) {
            summary.dated_docs += 1;
        } else {
            let c = scan_doc_text_for_drift(
                &text,
                doc_path,
                file_map,
                sym_set,
                ingest,
                interner,
                drift_items,
            );
            summary.prose_docs += usize::from(c.checked == 0);
            summary.anchors += c.total;
            summary.checked += c.checked;
        }
    }
    summary
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
    let summary = scan_corpus_markdown_docs(
        root_path,
        ingest,
        interner,
        &file_map,
        &sym_set,
        &mut drift_items,
    );
    filter_present_in_code_drift(root_path, ingest, interner, &mut drift_items);
    if with_history {
        let head_sha = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
        for item in &mut drift_items {
            enrich_drift_item_history(root, item, &head_sha);
        }
    }
    let drifted_docs: std::collections::HashSet<&str> = drift_items
        .iter()
        .filter_map(|d| d["doc"].as_str())
        .collect();
    let clean_docs = summary
        .docs_scanned
        .saturating_sub(summary.dated_docs)
        .saturating_sub(drifted_docs.len());
    let unchecked = summary.anchors.saturating_sub(summary.checked);

    json!({
        "root": root,
        "docs_scanned": summary.docs_scanned,
        "clean_docs": clean_docs,
        "dated_docs": summary.dated_docs,
        "prose_docs": summary.prose_docs,
        "corpus": ingest.files.len(),
        "anchors": summary.anchors,
        "checked": summary.checked,
        "unchecked": unchecked,
        "drift_count": drift_items.len(),
        "drift": drift_items
    })
}

pub fn extract_env_identifier(line: &str) -> Option<String> {
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

pub fn extract_compile_guard(line: &str) -> Option<String> {
    let trimmed = line.trim();
    for prefix in ["#ifndef ", "#ifdef "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let name: String = rest
                .split_whitespace()
                .next()?
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty()
                && !name.ends_with("_H")
                && !name.ends_with("_HPP")
                && !name.ends_with("_INCLUDED")
            {
                return Some(name);
            }
        }
    }
    None
}

fn find_ifndef_default(lines: &[&str], idx: usize, name: &str) -> Option<(String, bool)> {
    for &next_line in lines.iter().skip(idx + 1).take(3) {
        if let Some(def_rest) = next_line.trim().strip_prefix("#define ") {
            let mut parts = def_rest.split_whitespace();
            if parts.next() == Some(name) {
                let val = parts.next()?;
                let dark = val == "0" || val.eq_ignore_ascii_case("false");
                return Some((val.to_string(), dark));
            }
        }
    }
    Some(("0".to_string(), true))
}

fn measure_preproc_region_loc(lines: &[&str], start_idx: usize) -> usize {
    let mut depth = 1usize;
    for (offset, &line) in lines.iter().skip(start_idx + 1).enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#if") {
            depth += 1;
        } else if trimmed.starts_with("#endif") {
            depth -= 1;
            if depth == 0 {
                return offset;
            }
        }
    }
    0
}

pub fn extract_compile_guard_with_default(
    lines: &[&str],
    idx: usize,
) -> Option<(String, String, bool)> {
    let name = extract_compile_guard(lines[idx])?;
    let (val, dark) = if lines[idx].trim().starts_with("#ifndef ") {
        find_ifndef_default(lines, idx, &name)?
    } else {
        ("0".to_string(), true)
    };
    Some((name, val, dark))
}

pub fn extract_cmake_option(line: &str, path: &str) -> Option<(String, String, bool)> {
    if !path.ends_with(".cmake") && !path.ends_with("CMakeLists.txt") {
        return None;
    }
    let rest = line.trim().strip_prefix("option(")?;
    let inner = rest.trim_end_matches(')').trim();
    let name: String = inner
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        return None;
    }
    let is_on = inner
        .split_whitespace()
        .last()
        .is_some_and(|t| t.eq_ignore_ascii_case("ON"));
    let def_val = if is_on { "ON" } else { "OFF" };
    Some((name, def_val.to_string(), !is_on))
}

#[derive(Default)]
struct GateEntry {
    name: String,
    kind: &'static str,
    default_val: String,
    is_dark: bool,
    file: String,
    line: usize,
    regions: usize,
    loc: usize,
    reads: Vec<(String, usize)>,
}

#[derive(Default)]
struct FlagsScanTally {
    by_key: std::collections::BTreeMap<(String, &'static str), GateEntry>,
}

impl FlagsScanTally {
    fn record_compile(
        &mut self,
        name: String,
        def_val: String,
        is_dark: bool,
        path: &str,
        line: usize,
        reg_loc: usize,
    ) {
        let entry = self
            .by_key
            .entry((name.clone(), "compile"))
            .or_insert_with(|| GateEntry {
                name,
                kind: "compile",
                default_val: def_val.clone(),
                is_dark,
                file: path.to_string(),
                line,
                regions: 0,
                loc: 0,
                reads: Vec::new(),
            });
        entry.regions += 1;
        entry.loc += reg_loc;
        entry.reads.push((path.to_string(), line));
    }

    fn record_cmake(
        &mut self,
        name: String,
        def_val: String,
        is_dark: bool,
        path: &str,
        line: usize,
    ) {
        let entry = self
            .by_key
            .entry((name.clone(), "cmake"))
            .or_insert_with(|| GateEntry {
                name,
                kind: "cmake",
                default_val: def_val,
                is_dark,
                file: path.to_string(),
                line,
                regions: 0,
                loc: 0,
                reads: Vec::new(),
            });
        entry.reads.push((path.to_string(), line));
    }

    fn record_env(&mut self, name: String, path: &str, line: usize) {
        let entry = self
            .by_key
            .entry((name.clone(), "env"))
            .or_insert_with(|| GateEntry {
                name,
                kind: "env",
                default_val: "unset".to_string(),
                is_dark: true,
                file: path.to_string(),
                line,
                regions: 0,
                loc: 0,
                reads: Vec::new(),
            });
        entry.reads.push((path.to_string(), line));
    }
}

fn scan_file_flags(root_path: &std::path::Path, path: &str, tally: &mut FlagsScanTally) {
    let Ok(text) = crate::domain::read_rel_string(root_path, path) else {
        return;
    };
    let lines: Vec<&str> = text.lines().collect();
    for (idx, &line) in lines.iter().enumerate() {
        if let Some((guard, def_val, is_dark)) = extract_compile_guard_with_default(&lines, idx) {
            let reg_loc = measure_preproc_region_loc(&lines, idx);
            tally.record_compile(guard, def_val, is_dark, path, idx + 1, reg_loc);
        } else if let Some((opt, def_val, is_dark)) = extract_cmake_option(line, path) {
            tally.record_cmake(opt, def_val, is_dark, path, idx + 1);
        } else if let Some(env_var) = extract_env_identifier(line) {
            tally.record_env(env_var, path, idx + 1);
        }
    }
}

/// Execute `flags` tool: detect build flags and runtime configuration gates.
pub fn call_flags(ingest: &IngestResult, interner: &Interner) -> Value {
    call_flags_in_root(".", ingest, interner)
}

pub fn call_flags_in_root(root: &str, ingest: &IngestResult, interner: &Interner) -> Value {
    let root_path = std::path::Path::new(root);
    let mut tally = FlagsScanTally::default();

    for file in &ingest.files {
        if !file.lang.semantics().is_prose_document() {
            let path = interner.resolve(&file.path);
            scan_file_flags(root_path, path, &mut tally);
        }
    }
    for cmake_rel in ["CMakeLists.txt", "cmake/options.cmake"] {
        if root_path.join(cmake_rel).is_file() {
            scan_file_flags(root_path, cmake_rel, &mut tally);
        }
    }

    let mut flags = Vec::with_capacity(tally.by_key.len());
    let mut compile_count = 0usize;
    let mut cmake_count = 0usize;
    let mut env_count = 0usize;
    let mut dark_gates = 0usize;
    for entry in tally.by_key.into_values() {
        match entry.kind {
            "compile" => compile_count += 1,
            "cmake" => cmake_count += 1,
            _ => env_count += 1,
        }
        dark_gates += usize::from(entry.is_dark);
        let reads_json: Vec<Value> = entry
            .reads
            .iter()
            .map(|(p, l)| json!({ "path": p, "line": l }))
            .collect();
        flags.push(json!({
            "name": entry.name,
            "flag": entry.name,
            "kind": entry.kind,
            "default": entry.default_val,
            "dark": usize::from(entry.is_dark),
            "regions": entry.regions,
            "loc": entry.loc,
            "reads_count": entry.reads.len(),
            "reads": reads_json,
            "file": entry.file,
            "path": entry.file,
            "line": entry.line
        }));
    }

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

pub fn parse_branch_grep_line(line: &str, branch: &str, symbol_name: &str) -> Option<Value> {
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

pub fn should_scan_branch(branch: &str, on_head: bool) -> bool {
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

pub fn count_rev_list(root: &str, base: &str, branch: &str) -> u64 {
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

pub fn inspect_branch_stray(root: &str, branch: &str, head_ref: &str) -> Option<(String, Value)> {
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
