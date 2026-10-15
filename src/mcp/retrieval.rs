use crate::domain::{IngestResult, Interner, RefRole, SymKind};
use crate::graph::Graph;
use serde_json::{Value, json};
use std::fs;

fn stem_query_term(term: &str) -> String {
    let t = term.to_ascii_lowercase();
    if t.len() >= 5 && t.ends_with('s') && !t.ends_with("ss") {
        t[..t.len() - 1].to_string()
    } else {
        t
    }
}

/// Score a symbol against a single search term across name, scope, and file path.
fn score_single_term(
    lower_name: &str,
    lower_scope: Option<&str>,
    lower_path: &str,
    term: &str,
) -> f64 {
    let t = term.to_ascii_lowercase();
    let stem = stem_query_term(&t);
    let name_hit = if lower_name == t || lower_name == stem {
        30.0
    } else if lower_name.contains(&t) || lower_name.contains(&stem) {
        12.0
    } else {
        0.0
    };
    let scope_hit = if lower_scope.is_some_and(|sc| sc.contains(&t) || sc.contains(&stem)) {
        6.0
    } else {
        0.0
    };
    let path_hit =
        if !lower_path.is_empty() && (lower_path.contains(&t) || lower_path.contains(&stem)) {
            4.0
        } else {
            0.0
        };
    name_hit + scope_hit + path_hit
}

fn build_body_term_masks(
    ingest: &IngestResult,
    interner: &Interner,
    raw_terms: &[&str],
) -> Vec<u32> {
    let mut masks = vec![0u32; ingest.symbols.len()];
    if raw_terms.is_empty() {
        return masks;
    }
    let stems: Vec<(String, String)> = raw_terms
        .iter()
        .take(16)
        .map(|&t| {
            let low = t.to_ascii_lowercase();
            let st = stem_query_term(&low);
            (low, st)
        })
        .collect();
    for r in &ingest.references {
        let sym_idx = r.from_symbol as usize;
        if sym_idx >= masks.len() {
            continue;
        }
        let callee = interner.resolve(&r.callee_name).to_ascii_lowercase();
        for (i, (low, st)) in stems.iter().enumerate() {
            if callee.contains(low) || callee.contains(st) {
                masks[sym_idx] |= 1u32 << i;
            }
        }
    }
    masks
}

fn score_symbol_match_in_file(
    name: &str,
    scope: Option<&str>,
    file_path: &str,
    body_mask: u32,
    terms: &[&str],
    rank: f64,
) -> f64 {
    let lower_name = name.to_ascii_lowercase();
    let lower_scope = scope.map(|s| s.to_ascii_lowercase());
    let lower_path = file_path.to_ascii_lowercase();
    let scores: Vec<f64> = terms
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            let base = score_single_term(&lower_name, lower_scope.as_deref(), &lower_path, t);
            let body = if (body_mask & (1u32 << i.min(31))) != 0 {
                6.0
            } else {
                0.0
            };
            base + body
        })
        .filter(|&s| s > 0.0)
        .collect();
    if !scores.iter().any(|&s| s > 6.0) {
        return 0.0;
    }
    let term_score: f64 = scores.iter().sum();
    let synergy = (scores.len() as f64).powi(2);
    let non_test = if crate::gate::affected::is_test_path(file_path) {
        0.25
    } else {
        1.0
    };
    term_score * synergy * non_test * (1.0 + rank * 10.0)
}

fn score_candidate_symbol(
    sym: &crate::domain::Symbol,
    ingest: &IngestResult,
    interner: &Interner,
    ranks: &[f64],
    body_masks: &[u32],
    raw_terms: &[&str],
) -> f64 {
    if sym.kind == SymKind::Section {
        return 0.0;
    }
    let name = interner.resolve(&sym.name);
    let scope = sym.scope.as_ref().map(|s| interner.resolve(s));
    let file_path = ingest
        .files
        .get(sym.file_id as usize)
        .map_or("", |f| interner.resolve(&f.path));
    let rank = ranks.get(sym.id as usize).copied().unwrap_or(0.0);
    let body_mask = body_masks.get(sym.id as usize).copied().unwrap_or(0);
    let kind_factor = if matches!(sym.kind, SymKind::Var | SymKind::Field) {
        0.2
    } else {
        1.0
    };
    score_symbol_match_in_file(name, scope, file_path, body_mask, raw_terms, rank) * kind_factor
}

fn extract_normalized_sig(
    sym: &crate::domain::Symbol,
    bytes: &[u8],
    fallback_name: &str,
) -> String {
    let start = (sym.sig_start_byte as usize).min(bytes.len());
    let end = (sym.sig_end_byte as usize).min(bytes.len());
    if start >= end {
        return fallback_name.to_string();
    }
    let raw = String::from_utf8_lossy(&bytes[start..end]);
    let trimmed = raw.trim().trim_end_matches('{').trim();
    let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        fallback_name.to_string()
    } else {
        collapsed
    }
}

fn read_file_cached<'a>(
    root: &str,
    rel_path: &str,
    file_id: u32,
    cache: &'a mut std::collections::HashMap<u32, Vec<u8>>,
) -> &'a [u8] {
    cache.entry(file_id).or_insert_with(|| {
        let candidate = std::path::Path::new(root).join(rel_path);
        fs::read(&candidate)
            .or_else(|_| fs::read(rel_path))
            .unwrap_or_default()
    })
}

#[derive(Default, Clone, Copy)]
pub struct ForTaskOptions<'a> {
    pub root: &'a str,
    pub graph: Option<&'a Graph>,
    pub budget_tokens: Option<usize>,
    pub detail: Option<usize>,
    pub adaptive: bool,
}

/// Execute `for` task-lens ranking tool.
pub fn compute_adaptive_cut(scores: &[f64], floor: usize, ceiling: usize) -> (usize, usize, f64) {
    let limit = ceiling.min(scores.len());
    if limit <= floor {
        return (limit, limit, 0.0);
    }
    let mut max_gap = 0.0;
    let mut best_k = limit;
    for k in floor..limit {
        let prev = scores[k - 1];
        let curr = scores[k];
        if prev > 0.0 {
            let gap = (prev - curr) / prev;
            if gap > max_gap {
                max_gap = gap;
                best_k = k;
            }
        }
    }
    if max_gap >= 0.25 {
        (best_k, best_k, max_gap)
    } else {
        (limit, limit, max_gap)
    }
}

pub fn call_for_task(
    ingest: &IngestResult,
    interner: &Interner,
    ranks: &[f64],
    task: &str,
    budget_tokens: Option<usize>,
    detail: Option<usize>,
    adaptive: bool,
) -> Value {
    call_for_task_with_options(
        ingest,
        interner,
        ranks,
        task,
        ForTaskOptions {
            root: ".",
            graph: None,
            budget_tokens,
            detail,
            adaptive,
        },
    )
}

fn build_for_symbol_entry(
    idx: usize,
    score: f64,
    sym: &crate::domain::Symbol,
    path: &str,
    name: &str,
    opts: ForTaskOptions<'_>,
    file_cache: &mut std::collections::HashMap<u32, Vec<u8>>,
) -> Value {
    let detail_count = opts.detail.unwrap_or(0);
    let bytes = read_file_cached(opts.root, path, sym.file_id, file_cache);
    let sig = extract_normalized_sig(sym, bytes, name);
    let in_deg = opts.graph.map_or(0, |g| g.in_degree(sym.id));
    let end_line = sym.line.saturating_add(sym.loc.saturating_sub(1));
    let mut obj = json!({
        "name": name,
        "kind": sym.kind.tag(),
        "path": path,
        "line": sym.line,
        "end_line": end_line,
        "sig": sig,
        "cx": sym.cx,
        "ccx": sym.ccx,
        "in": in_deg,
        "amp": in_deg.max(1),
        "score": (score * 100.0).round() / 100.0
    });
    if idx < detail_count && !bytes.is_empty() {
        let start = (sym.sig_start_byte as usize).min(bytes.len());
        let end = (sym.end_byte as usize).min(bytes.len());
        if start < end {
            let body = String::from_utf8_lossy(&bytes[start..end]).to_string();
            obj["body"] = Value::String(body);
        }
    }
    obj
}

fn compute_for_route_metadata(
    task: &str,
    raw_terms: &[&str],
    scored: &[(f64, &crate::domain::Symbol)],
    interner: &Interner,
) -> (String, &'static str, u64, u64) {
    let exact_hit = raw_terms.len() == 1
        && scored
            .first()
            .is_some_and(|(_, s)| interner.resolve(&s.name).eq_ignore_ascii_case(raw_terms[0]));
    let route = if exact_hit {
        format!("name-exact({task})")
    } else if raw_terms.len() <= 2 && !scored.is_empty() {
        "subtoken+body:focused".to_string()
    } else {
        "subtoken+body:broad".to_string()
    };
    let margin_pct = if scored.len() >= 2 && scored[0].0 > 0.0 {
        (((scored[0].0 - scored[1].0) / scored[0].0) * 100.0).round() as u64
    } else {
        u64::from(scored.len() == 1) * 100
    };
    let confidence = if exact_hit || margin_pct >= 25 {
        "high"
    } else if !scored.is_empty() {
        "medium"
    } else {
        "low"
    };
    let matched_terms = raw_terms
        .iter()
        .filter(|&&t| {
            let lower_t = t.to_ascii_lowercase();
            scored.iter().take(10).any(|(_, s)| {
                interner
                    .resolve(&s.name)
                    .to_ascii_lowercase()
                    .contains(&lower_t)
            })
        })
        .count();
    let coverage = if raw_terms.is_empty() {
        100
    } else {
        ((matched_terms * 100) / raw_terms.len()) as u64
    };
    (route, confidence, margin_pct, coverage)
}

pub fn call_for_task_with_options(
    ingest: &IngestResult,
    interner: &Interner,
    ranks: &[f64],
    task: &str,
    opts: ForTaskOptions<'_>,
) -> Value {
    let raw_terms: Vec<&str> = task
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() >= 2)
        .collect();

    let body_masks = build_body_term_masks(ingest, interner, &raw_terms);
    let mut scored = Vec::new();
    for sym in &ingest.symbols {
        let score = score_candidate_symbol(sym, ingest, interner, ranks, &body_masks, &raw_terms);
        if score > 0.0 {
            scored.push((score, sym));
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut limit = opts.budget_tokens.unwrap_or(25).min(scored.len());
    let mut adaptive_info = None;
    if opts.adaptive && scored.len() > 5 {
        let scores: Vec<f64> = scored.iter().map(|s| s.0).collect();
        let (kept, _cliff, gap) = compute_adaptive_cut(&scores, 5, limit);
        adaptive_info = Some((kept, scored.len(), gap));
        limit = kept;
    }
    let detail_count = opts.detail.unwrap_or(0).min(limit);
    let mut file_cache = std::collections::HashMap::new();
    let symbols_json: Vec<Value> = scored[..limit]
        .iter()
        .enumerate()
        .map(|(idx, (score, sym))| {
            let name = interner.resolve(&sym.name);
            let path = ingest
                .files
                .get(sym.file_id as usize)
                .map_or("", |f| interner.resolve(&f.path));
            build_for_symbol_entry(idx, *score, sym, path, name, opts, &mut file_cache)
        })
        .collect();

    let (route, confidence, margin_pct, coverage) =
        compute_for_route_metadata(task, &raw_terms, &scored, interner);
    let est_tokens = (symbols_json.len() * 22 + detail_count * 120 + 180) as u64;
    json!({
        "task": task,
        "root": opts.root,
        "route": route,
        "confidence": confidence,
        "margin_pct": margin_pct,
        "coverage": coverage,
        "est_tokens": est_tokens,
        "symbols": symbols_json,
        "count": symbols_json.len(),
        "detail": detail_count,
        "adaptive": adaptive_info.map(|(k, t, g)| json!({ "kept": k, "total": t, "gap": g }))
    })
}

/// Execute `fetch_body` tool.
pub fn call_fetch_body(
    ingest: &IngestResult,
    interner: &Interner,
    symbol_name: &str,
    path_hint: Option<&str>,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Value {
    call_fetch_body_in_root(
        ".",
        ingest,
        interner,
        symbol_name,
        path_hint,
        start_line,
        end_line,
    )
}

pub fn call_fetch_body_in_root(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    symbol_name: &str,
    path_hint: Option<&str>,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Value {
    let sym = ingest.symbols.iter().find(|s| {
        let n = interner.resolve(&s.name);
        if n == symbol_name {
            if let Some(hint) = path_hint
                && let Some(fi) = ingest.files.get(s.file_id as usize)
            {
                return interner.resolve(&fi.path).contains(hint);
            }
            return true;
        }
        false
    });

    let Some(sym) = sym else {
        return json!({ "error": format!("symbol '{symbol_name}' not found") });
    };

    let Some(file_info) = ingest.files.get(sym.file_id as usize) else {
        return json!({ "error": "file info missing" });
    };

    let file_path = interner.resolve(&file_info.path);
    let joined = std::path::Path::new(root).join(file_path);
    let bytes = match fs::read(&joined).or_else(|_| fs::read(file_path)) {
        Ok(b) => b,
        Err(e) => return json!({ "error": format!("cannot read file '{file_path}': {e}") }),
    };

    let start = (sym.sig_start_byte as usize).min(bytes.len());
    let end = (sym.end_byte as usize).min(bytes.len());
    let body_str = String::from_utf8_lossy(&bytes[start..end]);
    let lines: Vec<&str> = body_str.lines().collect();
    let total_lines = lines.len();

    let s_line = start_line.unwrap_or(1).saturating_sub(1);
    let e_line = end_line.unwrap_or(total_lines).min(total_lines);

    let content = if s_line < e_line && s_line < lines.len() {
        lines[s_line..e_line].join("\n")
    } else {
        body_str.to_string()
    };

    json!({
        "symbol": symbol_name,
        "path": file_path,
        "line": sym.line,
        "total_lines": total_lines,
        "start_line": s_line + 1,
        "end_line": e_line,
        "body": content
    })
}

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

#[cfg(test)]
fn target_kind_from_role(role: &str) -> SymKind {
    exact_kind_from_token(role).unwrap_or(SymKind::Function)
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

    for &sym in candidates_pool {
        let file_path = ingest
            .files
            .get(sym.file_id as usize)
            .map(|f| interner.resolve(&f.path))
            .unwrap_or("");
        let is_non_test = !crate::gate::affected::is_test_path(file_path);
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
    let name = interner.resolve(&sym.name);
    let file_path = ingest
        .files
        .get(sym.file_id as usize)
        .map(|f| interner.resolve(&f.path))
        .unwrap_or("");

    let joined = std::path::Path::new(root).join(file_path);
    let body = fs::read(&joined)
        .or_else(|_| fs::read(file_path))
        .ok()
        .and_then(|bytes| {
            let start = (sym.sig_start_byte as usize).min(bytes.len());
            let end = (sym.end_byte as usize).min(bytes.len());
            if start < end {
                Some(String::from_utf8_lossy(&bytes[start..end]).to_string())
            } else {
                None
            }
        });

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
        "body": body.unwrap_or_default()
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

pub fn call_lego(ingest: &IngestResult, interner: &Interner, interface_name: &str) -> Value {
    let iface_sym = ingest.symbols.iter().find(|s| {
        interner.resolve(&s.name) == interface_name
            && (s.kind == SymKind::Interface
                || s.kind == SymKind::Class
                || s.kind == SymKind::Struct)
    });

    let mut implementors = Vec::new();
    let mut seen_impls = std::collections::HashSet::new();
    for r in &ingest.references {
        if r.role == RefRole::Extends
            && interner.resolve(&r.callee_name) == interface_name
            && let Some(src_sym) = resolve_implementor_symbol(ingest, r)
            && seen_impls.insert(src_sym.id)
        {
            let name = interner.resolve(&src_sym.name);
            let path = ingest
                .files
                .get(src_sym.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
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
            && let Ok(content) =
                fs::read_to_string(root_path.join(path)).or_else(|_| fs::read_to_string(path))
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

    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        if let Some(pf) = path_filter
            && !path.contains(pf)
        {
            continue;
        }

        let Ok(bytes) = fs::read(root_path.join(path)).or_else(|_| fs::read(path)) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };

        let all_lines: Vec<&str> = text.lines().collect();
        for (line_idx, &line) in all_lines.iter().enumerate() {
            if line.contains(pattern) {
                let line_num = line_idx as u32 + 1;
                let enc_sym = ingest
                    .symbols
                    .iter()
                    .filter(|s| s.file_id == file.id && s.line <= line_num)
                    .max_by_key(|s| s.line);
                let enclosing = enc_sym.map(|s| interner.resolve(&s.name));
                let enc_cx = enc_sym.map_or(1, |s| s.cx.max(1));
                let enc_callers = enc_sym.map_or(0, |s| {
                    ingest
                        .references
                        .iter()
                        .filter(|r| r.callee_name == s.name)
                        .count()
                });

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

fn match_pattern_on_text(text: &str, tokens: &[&str]) -> Vec<(u32, String)> {
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
        let Ok(bytes) = fs::read(root_path.join(path)).or_else(|_| fs::read(path)) else {
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

fn collect_query_file_hits(
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
        let Ok(bytes) = fs::read(root_path.join(path)).or_else(|_| fs::read(path)) else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_symbol_match() {
        let terms = ["search", "engine"];
        let score =
            score_symbol_match_in_file("search_engine", None, "src/search.rs", 0b10, &terms, 0.5);
        assert!(score > 0.0);
    }

    #[test]
    fn test_call_exemplar_roles() {
        let interner = Interner::new();
        let ingest = IngestResult::new();
        let res = call_exemplar(&ingest, &interner, &[], "fn", None);
        assert!(res.get("error").is_some());
        assert_eq!(target_kind_from_role("method"), SymKind::Method);
        assert_eq!(target_kind_from_role("class"), SymKind::Class);
        assert_eq!(target_kind_from_role("struct"), SymKind::Struct);
        assert_eq!(target_kind_from_role("interface"), SymKind::Interface);
        assert_eq!(target_kind_from_role("section"), SymKind::Section);
        assert_eq!(target_kind_from_role("other"), SymKind::Function);
    }

    #[test]
    fn test_call_for_task_with_detail() {
        let interner = Interner::new();
        let mut ingest = IngestResult::new();
        let sym = crate::domain::Symbol::new(
            0,
            interner.intern("test_fn"),
            interner.intern("c_test_fn"),
            SymKind::Function,
            crate::domain::Lang::Rust,
            0,
            1,
        );
        ingest.symbols.push(sym);
        let res = call_for_task(&ingest, &interner, &[1.0], "test fn", None, Some(1), false);
        assert_eq!(res["count"], 1);
        assert_eq!(res["detail"], 1);
    }

    #[test]
    fn test_compute_adaptive_cut() {
        assert_eq!(compute_adaptive_cut(&[10.0, 5.0], 5, 10), (2, 2, 0.0));
        let scores = [10.0, 9.0, 8.0, 2.0, 1.0];
        let (k, shown, gap) = compute_adaptive_cut(&scores, 1, 5);
        assert_eq!(k, 3);
        assert_eq!(shown, 3);
        assert!(gap >= 0.25);
        let no_drop = [10.0, 9.9, 9.8, 9.7];
        let (k_all, _, _) = compute_adaptive_cut(&no_drop, 1, 4);
        assert_eq!(k_all, 4);
    }

    #[test]
    fn test_pattern_and_ast_match() {
        let matches = match_pattern_on_text(
            "pub class Foo { bar(); }\nfn other() {}",
            &["class", "{", "}"],
        );
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, 1);

        let reg = crate::ingest::LanguageRegistry::new().unwrap();
        let interner = Interner::new();
        let code = b"pub struct Widget {}\npub fn build() {}";
        let ingest = crate::ingest::ingest_in_memory_files(
            &[("src/lib.rs", crate::domain::Lang::Rust, code)],
            &reg,
            &interner,
        );
        let hits = collect_query_file_hits(
            &ingest.files[0],
            code,
            "(struct_item name: (type_identifier) @name)",
            &ingest,
            &interner,
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0]["text"], "Widget");
    }
}
