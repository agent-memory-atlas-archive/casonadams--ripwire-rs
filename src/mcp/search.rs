use crate::domain::{IngestResult, Interner, RefRole, SymKind};
use crate::graph::Graph;
use serde_json::{Value, json};

/// Execute `exemplar` tool: get top-ranked symbol of a given kind to imitate.
fn exact_kind_from_token(role: &str) -> Option<SymKind> {
    match role.trim().to_ascii_lowercase().as_str() {
        "fn" | "function" => Some(SymKind::Function),
        "method" => Some(SymKind::Method),
        "class" | "cls" => Some(SymKind::Class),
        "struct" => Some(SymKind::Struct),
        "interface" | "iface" => Some(SymKind::Interface),
        "section" | "sec" => Some(SymKind::Section),
        "var" => Some(SymKind::Var),
        _ => None,
    }
}

fn filter_exemplar_candidates<'a>(
    ingest: &'a IngestResult,
    interner: &Interner,
    role: &str,
) -> (Vec<&'a crate::domain::Symbol>, bool) {
    if let Some(target_kind) = exact_kind_from_token(role) {
        let cands = ingest
            .symbols
            .iter()
            .filter(|s| s.kind == target_kind && s.end_byte > s.sig_start_byte)
            .collect();
        return (cands, false);
    }
    let terms: Vec<String> = role
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_ascii_lowercase())
        .collect();
    let topical: Vec<&crate::domain::Symbol> = ingest
        .symbols
        .iter()
        .filter(|s| {
            s.kind != SymKind::Section
                && s.kind != SymKind::Other
                && s.end_byte > s.sig_start_byte
                && {
                    let name_lower = interner.resolve(&s.name).to_ascii_lowercase();
                    terms.iter().any(|t| name_lower.contains(t))
                }
        })
        .collect();
    if !topical.is_empty() {
        return (topical, false);
    }
    let fallback = ingest
        .symbols
        .iter()
        .filter(|s| s.kind == SymKind::Function && s.end_byte > s.sig_start_byte)
        .collect();
    (fallback, true)
}

fn select_best_exemplar<'a>(
    ingest: &'a IngestResult,
    interner: &Interner,
    ranks: &[f64],
    graph: Option<&Graph>,
    role: &str,
) -> Option<(&'a crate::domain::Symbol, usize, usize, f64, bool)> {
    let (valid, low_conf) = filter_exemplar_candidates(ingest, interner, role);
    if valid.is_empty() {
        return None;
    }
    let eligible_under_cx: Vec<&crate::domain::Symbol> =
        valid.iter().copied().filter(|s| s.ccx <= 15).collect();
    let candidates_pool = if eligible_under_cx.is_empty() {
        &valid
    } else {
        &eligible_under_cx
    };
    let candidate_count = candidates_pool.len();
    let mut best: Option<(&crate::domain::Symbol, usize, f64)> = None;
    let mut best_score = (false, 0usize, i64::MIN, 0.0f64);

    use crate::domain::CorpusView;
    let view = (ingest, interner);
    for &sym in candidates_pool {
        let is_non_test = !view.is_test_symbol(sym);
        let in_deg = graph.map(|g| g.in_degree(sym.id)).unwrap_or(0);
        let neg_ccx = -(sym.ccx as i64);
        let r = ranks.get(sym.id as usize).copied().unwrap_or(0.0);
        let score = (is_non_test, in_deg, neg_ccx, r);
        if best.is_none() || score > best_score {
            best_score = score;
            best = Some((sym, in_deg, r));
        }
    }
    best.map(|(sym, in_deg, r)| (sym, in_deg, candidate_count, r, low_conf))
}

/// Execute `exemplar` tool: get top-ranked symbol of a given kind to imitate.
pub fn call_exemplar(
    ingest: &IngestResult,
    interner: &Interner,
    ranks: &[f64],
    role: &str,
    graph: Option<&Graph>,
) -> Value {
    call_exemplar_in_root(".", ingest, interner, ranks, role, graph)
}

fn collect_exemplar_calls(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    graph: Option<&Graph>,
    sym_id: u32,
) -> Vec<Value> {
    use crate::domain::CorpusView;
    let Some(g) = graph else {
        return Vec::new();
    };
    let view = (ingest, interner);
    g.callees(sym_id)
        .filter_map(|(cid, _)| ingest.symbols.get(cid as usize))
        .map(|csym| {
            let sig = view.read_sym_signature(root, csym).unwrap_or_default();
            json!({
                "name": view.sym_name(csym),
                "line": csym.line,
                "sig": sig
            })
        })
        .collect()
}

pub fn call_exemplar_in_root(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    ranks: &[f64],
    role: &str,
    graph: Option<&Graph>,
) -> Value {
    let Some((sym, in_deg, cand_cnt, rank, low_conf)) =
        select_best_exemplar(ingest, interner, ranks, graph, role)
    else {
        return json!({ "error": format!("no exemplar found for role '{role}'") });
    };
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let name = view.sym_name(sym);
    let file_path = view.sym_path(sym);
    let body = view.read_sym_source(root, sym);
    let calls = collect_exemplar_calls(root, ingest, interner, graph, sym.id);
    json!({
        "role": role,
        "kind": sym.kind.tag(),
        "low_confidence": usize::from(low_conf),
        "candidates": cand_cnt,
        "name": name,
        "path": file_path,
        "line": sym.line,
        "in": in_deg,
        "callers": in_deg,
        "ccx": sym.ccx,
        "tested": usize::from(in_deg > 0),
        "rank": (rank * 10000.0).round() / 10000.0,
        "body": body.unwrap_or_default(),
        "calls": calls
    })
}

/// Execute `lego` tool: interface contract and its implementors.
fn resolve_implementor_symbol<'a>(
    ingest: &'a IngestResult,
    r: &crate::domain::Reference,
) -> Option<&'a crate::domain::Symbol> {
    if let Some(src_sym) = ingest.symbols.get(r.from_symbol as usize)
        && (src_sym.kind == SymKind::Class
            || src_sym.kind == SymKind::Struct
            || src_sym.kind == SymKind::Interface)
    {
        return Some(src_sym);
    }
    let q_spur = r.qualifier?;
    ingest.symbols.iter().find(|s| {
        s.name == q_spur
            && (s.kind == SymKind::Class
                || s.kind == SymKind::Struct
                || s.kind == SymKind::Interface)
    })
}

fn collect_interface_methods(
    ingest: &IngestResult,
    interner: &Interner,
    iface_file_id: Option<u32>,
    unique_def: bool,
    interface_name: &str,
) -> Vec<Value> {
    let Some(fid) = iface_file_id else {
        return Vec::new();
    };
    let mut seen_names = std::collections::HashSet::new();
    let mut methods = Vec::new();
    for s in &ingest.symbols {
        let in_scope = s
            .scope
            .is_some_and(|sc| interner.resolve(&sc) == interface_name);
        let is_member = (s.file_id == fid && matches!(s.kind, SymKind::Method | SymKind::Function))
            || (unique_def && s.kind == SymKind::Method);
        if in_scope && is_member && seen_names.insert(s.name) {
            methods.push(json!({
                "name": interner.resolve(&s.name),
                "line": s.line,
                "params": s.params
            }));
        }
    }
    methods
}

pub fn call_lego(ingest: &IngestResult, interner: &Interner, interface_name: &str) -> Value {
    let iface_defs: Vec<&crate::domain::Symbol> = ingest
        .symbols
        .iter()
        .filter(|s| {
            interner.resolve(&s.name) == interface_name
                && (s.kind == SymKind::Interface
                    || s.kind == SymKind::Class
                    || s.kind == SymKind::Struct)
        })
        .collect();
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let iface_sym = iface_defs.first().copied();
    let iface_path = iface_sym.map_or("", |s| view.sym_path(s));
    let methods = collect_interface_methods(
        ingest,
        interner,
        iface_sym.map(|s| s.file_id),
        iface_defs.len() <= 1,
        interface_name,
    );
    let mut implementors = Vec::new();
    let mut seen_impls = std::collections::HashSet::new();
    for r in &ingest.references {
        if r.role == RefRole::Extends
            && interner.resolve(&r.callee_name) == interface_name
            && let Some(src_sym) = resolve_implementor_symbol(ingest, r)
            && seen_impls.insert(src_sym.id)
        {
            let name = view.sym_name(src_sym);
            let path = view.sym_path(src_sym);
            implementors.push(json!({
                "name": name,
                "kind": src_sym.kind.tag(),
                "path": path,
                "line": src_sym.line
            }));
        }
    }

    json!({
        "interface": interface_name,
        "defined": iface_sym.is_some(),
        "defs": iface_defs.len().max(usize::from(iface_sym.is_some())),
        "path": iface_path,
        "methods": methods,
        "implementors": implementors,
        "count": implementors.len()
    })
}

/// Execute `memory_recall` tool: search docs for task terms.
pub fn call_memory_recall(
    ingest: &IngestResult,
    interner: &Interner,
    task: &str,
    top_k: Option<usize>,
) -> Value {
    call_memory_recall_in_root(".", ingest, interner, task, top_k)
}

pub fn call_memory_recall_in_root(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    task: &str,
    top_k: Option<usize>,
) -> Value {
    let terms: Vec<String> = task
        .split_whitespace()
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let root_path = std::path::Path::new(root);

    let mut matched_docs = Vec::new();
    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        if (path.ends_with(".md") || path.contains("doc") || path.contains("notes"))
            && let Ok(content) = crate::domain::read_rel_string(root_path, path)
        {
            let lower = content.to_ascii_lowercase();
            let score = terms.iter().filter(|&t| lower.contains(t)).count();
            if score > 0 {
                matched_docs.push((score, path, content));
            }
        }
    }

    matched_docs.sort_by_key(|b| std::cmp::Reverse(b.0));
    let limit = top_k.unwrap_or(5).min(matched_docs.len());

    let results: Vec<Value> = matched_docs[..limit]
        .iter()
        .map(|(score, path, text)| {
            let preview = text.lines().take(20).collect::<Vec<_>>().join("\n");
            json!({
                "path": path,
                "score": score,
                "content": preview
            })
        })
        .collect();

    json!({
        "task": task,
        "docs": results,
        "count": results.len()
    })
}

/// Execute `grep` tool: literal search annotated with enclosing symbol.
pub fn call_grep(
    ingest: &IngestResult,
    interner: &Interner,
    pattern: &str,
    path_filter: Option<&str>,
) -> Value {
    call_grep_in_root(".", ingest, interner, pattern, path_filter)
}

pub fn call_grep_in_root(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    pattern: &str,
    path_filter: Option<&str>,
) -> Value {
    call_grep_with_context(root, ingest, interner, pattern, path_filter, 0, 0)
}

fn build_grep_lookups(
    ingest: &IngestResult,
) -> (
    std::collections::HashMap<u32, Vec<&crate::domain::Symbol>>,
    std::collections::HashMap<lasso::Spur, usize>,
) {
    let mut syms_by_file: std::collections::HashMap<u32, Vec<&crate::domain::Symbol>> =
        std::collections::HashMap::new();
    for s in &ingest.symbols {
        if s.kind != SymKind::Section {
            syms_by_file.entry(s.file_id).or_default().push(s);
        }
    }
    let mut callers_by_name: std::collections::HashMap<lasso::Spur, usize> =
        std::collections::HashMap::new();
    for r in &ingest.references {
        *callers_by_name.entry(r.callee_name).or_default() += 1;
    }
    (syms_by_file, callers_by_name)
}

pub fn call_grep_with_context(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    pattern: &str,
    path_filter: Option<&str>,
    before_lines: usize,
    after_lines: usize,
) -> Value {
    let mut hits = Vec::new();
    let root_path = std::path::Path::new(root);
    let (syms_by_file, callers_by_name) = build_grep_lookups(ingest);

    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        if let Some(pf) = path_filter
            && !path.contains(pf)
        {
            continue;
        }

        let Ok(bytes) = crate::domain::read_rel_bytes(root_path, path) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };

        let file_syms = syms_by_file.get(&file.id);
        let all_lines: Vec<&str> = text.lines().collect();
        for (line_idx, &line) in all_lines.iter().enumerate() {
            if line.contains(pattern) {
                let line_num = line_idx as u32 + 1;
                let enc_sym = file_syms
                    .and_then(|syms| {
                        syms.iter()
                            .filter(|s| s.line <= line_num)
                            .max_by_key(|s| s.line)
                    })
                    .copied();
                let enclosing = enc_sym.map(|s| interner.resolve(&s.name));
                let enc_cx = enc_sym.map_or(1, |s| s.cx.max(1));
                let enc_callers = enc_sym
                    .and_then(|s| callers_by_name.get(&s.name).copied())
                    .unwrap_or(0);

                let b_start = line_idx.saturating_sub(before_lines);
                let before: Vec<Value> = (b_start..line_idx)
                    .map(|i| json!({"line": i + 1, "text": all_lines[i]}))
                    .collect();
                let a_end = (line_idx + 1 + after_lines).min(all_lines.len());
                let after: Vec<Value> = ((line_idx + 1)..a_end)
                    .map(|i| json!({"line": i + 1, "text": all_lines[i]}))
                    .collect();

                hits.push(json!({
                    "path": path,
                    "line": line_num,
                    "text": line.trim(),
                    "enclosing": enclosing,
                    "enclosing_callers": enc_callers,
                    "enclosing_cx": enc_cx,
                    "before": before,
                    "after": after
                }));
            }
        }
    }
    json!({
        "pattern": pattern,
        "hits": hits,
        "count": hits.len()
    })
}

fn line_matches_tokens(line: &str, tokens: &[&str]) -> bool {
    let Some((&first, rest)) = tokens.split_first() else {
        return false;
    };
    let Some(mut pos) = line.find(first) else {
        return false;
    };
    pos += first.len();
    for &tok in rest {
        let Some(rel) = line[pos..].find(tok) else {
            return false;
        };
        pos += rel + tok.len();
    }
    true
}

pub fn match_pattern_on_text(text: &str, tokens: &[&str]) -> Vec<(u32, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line_matches_tokens(line, tokens))
        .map(|(idx, line)| (idx as u32 + 1, line.trim().to_string()))
        .collect()
}

/// Execute `pattern` structural code-shaped search with `$NAME` / `$$$` wildcards.
pub fn call_pattern_search(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    pattern: &str,
    path_filter: Option<&str>,
) -> Value {
    let tokens: Vec<&str> = pattern
        .split_whitespace()
        .filter(|t| !t.starts_with('$'))
        .collect();
    let mut hits = Vec::new();
    let root_path = std::path::Path::new(root);

    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        if path_filter.is_some_and(|pf| !path.contains(pf)) {
            continue;
        }
        let Ok(bytes) = crate::domain::read_rel_bytes(root_path, path) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        for (line_num, snippet) in match_pattern_on_text(text, &tokens) {
            let enclosing = ingest
                .symbols
                .iter()
                .filter(|s| s.file_id == file.id && s.line <= line_num)
                .max_by_key(|s| s.line)
                .map(|s| interner.resolve(&s.name));
            hits.push(json!({
                "path": path,
                "line": line_num,
                "text": snippet,
                "enclosing": enclosing
            }));
        }
    }
    json!({
        "pattern": pattern,
        "hits": hits,
        "count": hits.len()
    })
}

pub fn collect_query_file_hits(
    file: &crate::domain::FileInfo,
    bytes: &[u8],
    query_str: &str,
    ingest: &IngestResult,
    interner: &Interner,
) -> Option<Vec<Value>> {
    use streaming_iterator::StreamingIterator;
    let reg = crate::ingest::LanguageRegistry::new().ok()?;
    let (ts_lang, _) = reg.get(file.lang)?;
    let path = interner.resolve(&file.path);
    let q = tree_sitter::Query::new(ts_lang, query_str).ok()?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(ts_lang).ok()?;
    let tree = parser.parse(bytes, None)?;
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&q, tree.root_node(), bytes);
    let mut hits = Vec::new();
    while let Some(m) = matches.next() {
        let Some(cap) = m.captures.first() else {
            continue;
        };
        let node = cap.node;
        let line_num = node.start_position().row as u32 + 1;
        let text = node
            .utf8_text(bytes)
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .trim();
        let enclosing = ingest
            .symbols
            .iter()
            .filter(|s| s.file_id == file.id && s.line <= line_num)
            .max_by_key(|s| s.line)
            .map(|s| interner.resolve(&s.name));
        hits.push(json!({
            "path": path,
            "line": line_num,
            "text": text,
            "enclosing": enclosing
        }));
    }
    Some(hits)
}

pub fn call_ast_match(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    query_str: &str,
) -> Result<Value, String> {
    let Ok(reg) = crate::ingest::LanguageRegistry::new() else {
        return Err("failed to initialize language registry".to_string());
    };
    let root_path = std::path::Path::new(root);
    let mut hits = Vec::new();
    let mut compiled_any = false;

    for file in &ingest.files {
        if let Some((ts_lang, _)) = reg.get(file.lang)
            && tree_sitter::Query::new(ts_lang, query_str).is_ok()
        {
            compiled_any = true;
        }
        let path = interner.resolve(&file.path);
        let Ok(bytes) = crate::domain::read_rel_bytes(root_path, path) else {
            continue;
        };
        if let Some(file_hits) = collect_query_file_hits(file, &bytes, query_str, ingest, interner)
        {
            hits.extend(file_hits);
        }
    }
    if !compiled_any && !ingest.files.is_empty() {
        return Err(format!(
            "ripwire: --match: invalid S-expression query '{query_str}'"
        ));
    }
    Ok(json!({
        "pattern": query_str,
        "hits": hits,
        "count": hits.len()
    }))
}
