pub use super::branch_tools::*;

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
    let Ok(entries) = fs::read_dir(root_path.join("src/queries")) else {
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

impl GateEntry {
    fn configure(&mut self, def_val: String, is_dark: bool) {
        if self.reads.len() == 1 {
            self.default_val = def_val;
            self.is_dark = is_dark;
        }
    }
}

#[derive(Default)]
struct FlagsScanTally {
    by_key: std::collections::BTreeMap<(String, &'static str), GateEntry>,
}

impl FlagsScanTally {
    fn record_site(
        &mut self,
        kind: &'static str,
        name: String,
        path: &str,
        line: usize,
    ) -> &mut GateEntry {
        let entry = self
            .by_key
            .entry((name.clone(), kind))
            .or_insert_with(|| GateEntry {
                name,
                kind,
                default_val: "unset".to_string(),
                is_dark: true,
                file: path.to_string(),
                line,
                regions: 0,
                loc: 0,
                reads: Vec::new(),
            });
        entry.reads.push((path.to_string(), line));
        entry
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
            let entry = tally.record_site("compile", guard, path, idx + 1);
            entry.configure(def_val, is_dark);
            entry.regions += 1;
            entry.loc += reg_loc;
        } else if let Some((opt, def_val, is_dark)) = extract_cmake_option(line, path) {
            tally
                .record_site("cmake", opt, path, idx + 1)
                .configure(def_val, is_dark);
        } else if let Some(env_var) = extract_env_identifier(line) {
            tally.record_site("env", env_var, path, idx + 1);
        }
    }
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
