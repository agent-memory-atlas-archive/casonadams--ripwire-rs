use crate::domain::{IngestResult, Interner, RefRole, SymKind};
use serde_json::{json, Value};
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
        } else if let Some(ref sc) = lower_scope {
            if sc.contains(term) {
                term_score += 5.0;
            }
        }
    }

    if term_score > 0.0 {
        term_score * (1.0 + rank * 10.0)
    } else {
        0.0
    }
}

/// Execute `for` task-lens ranking tool.
pub fn call_for_task(
    ingest: &IngestResult,
    interner: &Interner,
    ranks: &[f64],
    task: &str,
    budget_tokens: Option<usize>,
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
    let limit = budget_tokens.unwrap_or(25).min(scored.len());

    let symbols_json: Vec<Value> = scored[..limit]
        .iter()
        .map(|(score, sym)| {
            let name = interner.resolve(&sym.name);
            let path = ingest
                .files
                .get(sym.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            json!({
                "name": name,
                "kind": sym.kind.tag(),
                "path": path,
                "line": sym.line,
                "score": (score * 100.0).round() / 100.0
            })
        })
        .collect();

    json!({
        "task": task,
        "symbols": symbols_json,
        "count": symbols_json.len()
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
            if let Some(hint) = path_hint {
                if let Some(fi) = ingest.files.get(s.file_id as usize) {
                    return interner.resolve(&fi.path).contains(hint);
                }
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

/// Execute `exemplar` tool: get top-ranked symbol of a given kind to imitate.
pub fn call_exemplar(
    ingest: &IngestResult,
    interner: &Interner,
    ranks: &[f64],
    role: &str,
) -> Value {
    let target_kind = target_kind_from_role(role);
    let mut best: Option<(f64, &crate::domain::Symbol)> = None;
    for sym in &ingest.symbols {
        if sym.kind == target_kind && sym.end_byte > sym.sig_start_byte {
            let r = ranks.get(sym.id as usize).copied().unwrap_or(0.0);
            if best.is_none() || best.is_some_and(|(b_rank, _)| r > b_rank) {
                best = Some((r, sym));
            }
        }
    }

    let Some((rank, sym)) = best else {
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
        "name": name,
        "path": file_path,
        "line": sym.line,
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
        if r.role == RefRole::Extends && interner.resolve(&r.callee_name) == interface_name {
            if let Some(src_sym) = ingest.symbols.get(r.from_symbol as usize) {
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
        if path.ends_with(".md") || path.contains("doc") || path.contains("notes") {
            if let Ok(content) = fs::read_to_string(path) {
                let lower = content.to_ascii_lowercase();
                let score = terms.iter().filter(|&t| lower.contains(t)).count();
                if score > 0 {
                    matched_docs.push((score, path, content));
                }
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
        if let Some(pf) = path_filter {
            if !path.contains(pf) {
                continue;
            }
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
        let res = call_exemplar(&ingest, &interner, &[], "fn");
        assert!(res.get("error").is_some());
        assert_eq!(target_kind_from_role("method"), SymKind::Method);
        assert_eq!(target_kind_from_role("class"), SymKind::Class);
        assert_eq!(target_kind_from_role("struct"), SymKind::Struct);
        assert_eq!(target_kind_from_role("interface"), SymKind::Interface);
        assert_eq!(target_kind_from_role("section"), SymKind::Section);
        assert_eq!(target_kind_from_role("other"), SymKind::Function);
    }
}
