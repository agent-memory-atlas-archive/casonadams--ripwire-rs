use crate::domain::{IngestResult, Interner, RefRole, SymKind};
use crate::graph::Graph;
use serde_json::{Value, json};
use std::fs;

/// Score a symbol against a set of search terms.
fn score_symbol_match(name: &str, scope: Option<&str>, terms: &[&str], rank: f64) -> f64 {
    let lower_name = name.to_ascii_lowercase();
    let lower_scope = scope.map(|s| s.to_ascii_lowercase());

    let mut term_score = 0.0;
    for term in terms {
        if lower_name.contains(term) {
            term_score += 10.0;
            if lower_name == *term {
                term_score += 20.0;
            }
        } else if let Some(ref sc) = lower_scope
            && sc.contains(term)
        {
            term_score += 5.0;
        }
    }

    if term_score > 0.0 {
        term_score * (1.0 + rank * 10.0)
    } else {
        0.0
    }
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
    let raw_terms: Vec<&str> = task
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() >= 2)
        .collect();

    let mut scored = Vec::new();
    for sym in &ingest.symbols {
        let name = interner.resolve(&sym.name);
        let scope = sym.scope.as_ref().map(|s| interner.resolve(s));
        let rank = ranks.get(sym.id as usize).copied().unwrap_or(0.0);
        let score = score_symbol_match(name, scope, &raw_terms, rank);
        if score > 0.0 {
            scored.push((score, sym));
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut limit = budget_tokens.unwrap_or(25).min(scored.len());
    let mut adaptive_info = None;
    if adaptive && scored.len() > 5 {
        let scores: Vec<f64> = scored.iter().map(|s| s.0).collect();
        let (kept, _cliff, gap) = compute_adaptive_cut(&scores, 5, limit);
        adaptive_info = Some((kept, scored.len(), gap));
        limit = kept;
    }
    let detail_count = detail.unwrap_or(0).min(limit);
    let symbols_json: Vec<Value> = scored[..limit]
        .iter()
        .enumerate()
        .map(|(idx, (score, sym))| {
            let name = interner.resolve(&sym.name);
            let path = ingest
                .files
                .get(sym.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            let mut obj = json!({
                "name": name,
                "kind": sym.kind.tag(),
                "path": path,
                "line": sym.line,
                "score": (score * 100.0).round() / 100.0
            });
            if idx < detail_count
                && let Ok(bytes) = fs::read(path)
            {
                let start = (sym.sig_start_byte as usize).min(bytes.len());
                let end = (sym.end_byte as usize).min(bytes.len());
                let body = String::from_utf8_lossy(&bytes[start..end]).to_string();
                obj["body"] = Value::String(body);
            }
            obj
        })
        .collect();

    json!({
        "task": task,
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
    let bytes = match fs::read(file_path) {
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
fn target_kind_from_role(role: &str) -> SymKind {
    match role {
        "method" => SymKind::Method,
        "class" | "cls" => SymKind::Class,
        "struct" => SymKind::Struct,
        "interface" | "iface" => SymKind::Interface,
        "section" | "sec" => SymKind::Section,
        _ => SymKind::Function,
    }
}
fn select_best_exemplar<'a>(
    ingest: &'a IngestResult,
    interner: &Interner,
    ranks: &[f64],
    graph: Option<&Graph>,
    target_kind: SymKind,
) -> Option<(&'a crate::domain::Symbol, usize, usize, f64)> {
    let valid: Vec<&crate::domain::Symbol> = ingest
        .symbols
        .iter()
        .filter(|s| s.kind == target_kind && s.end_byte > s.sig_start_byte)
        .collect();
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
    best.map(|(sym, in_deg, r)| (sym, in_deg, candidate_count, r))
}

/// Execute `exemplar` tool: get top-ranked symbol of a given kind to imitate.
pub fn call_exemplar(
    ingest: &IngestResult,
    interner: &Interner,
    ranks: &[f64],
    role: &str,
    graph: Option<&Graph>,
) -> Value {
    let target_kind = target_kind_from_role(role);
    let Some((sym, in_deg, cand_cnt, rank)) =
        select_best_exemplar(ingest, interner, ranks, graph, target_kind)
    else {
        return json!({ "error": format!("no exemplar found for role '{role}'") });
    };
    let name = interner.resolve(&sym.name);
    let file_path = ingest
        .files
        .get(sym.file_id as usize)
        .map(|f| interner.resolve(&f.path))
        .unwrap_or("");

    let body = fs::read(file_path).ok().and_then(|bytes| {
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
        "kind": role,
        "candidates": cand_cnt,
        "name": name,
        "path": file_path,
        "line": sym.line,
        "in": in_deg,
        "callers": in_deg,
        "ccx": sym.ccx,
        "tested": if in_deg > 0 { 1 } else { 0 },
        "rank": (rank * 10000.0).round() / 10000.0,
        "body": body.unwrap_or_default()
    })
}

/// Execute `lego` tool: interface contract and its implementors.
pub fn call_lego(ingest: &IngestResult, interner: &Interner, interface_name: &str) -> Value {
    let iface_sym = ingest.symbols.iter().find(|s| {
        interner.resolve(&s.name) == interface_name
            && (s.kind == SymKind::Interface
                || s.kind == SymKind::Class
                || s.kind == SymKind::Struct)
    });

    let mut implementors = Vec::new();
    for r in &ingest.references {
        if r.role == RefRole::Extends
            && interner.resolve(&r.callee_name) == interface_name
            && let Some(src_sym) = ingest.symbols.get(r.from_symbol as usize)
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
    let terms: Vec<String> = task
        .split_whitespace()
        .map(|s| s.to_ascii_lowercase())
        .collect();

    let mut matched_docs = Vec::new();
    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        if (path.ends_with(".md") || path.contains("doc") || path.contains("notes"))
            && let Ok(content) = fs::read_to_string(path)
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
    let mut hits = Vec::new();

    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        if let Some(pf) = path_filter
            && !path.contains(pf)
        {
            continue;
        }

        let Ok(bytes) = fs::read(path) else { continue };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };

        for (line_idx, line) in text.lines().enumerate() {
            if line.contains(pattern) {
                let line_num = line_idx as u32 + 1;
                let enclosing = ingest
                    .symbols
                    .iter()
                    .filter(|s| s.file_id == file.id && s.line <= line_num)
                    .max_by_key(|s| s.line)
                    .map(|s| interner.resolve(&s.name));

                hits.push(json!({
                    "path": path,
                    "line": line_num,
                    "text": line.trim(),
                    "enclosing": enclosing
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_symbol_match() {
        let terms = ["search", "engine"];
        let score = score_symbol_match("search_engine", None, &terms, 0.5);
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
}
