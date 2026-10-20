pub use super::search::*;

use crate::domain::{IngestResult, Interner, SymKind};
use crate::graph::Graph;
use serde_json::{Value, json};

fn stem_derivational_suffix(t: &str) -> Option<String> {
    if let Some(prefix) = t.strip_suffix("ution").filter(|p| p.len() >= 3) {
        return Some(format!("{prefix}v"));
    }
    for (suffix, min_stem) in [
        ("ization", 4),
        ("ation", 4),
        ("ition", 4),
        ("tion", 4),
        ("sion", 4),
        ("ment", 4),
        ("ing", 4),
        ("y", 5),
    ] {
        if let Some(stripped) = t.strip_suffix(suffix)
            && stripped.len() >= min_stem
        {
            return Some(stripped.to_string());
        }
    }
    None
}

fn stem_plural_only(term: &str) -> String {
    let mut t = term.to_ascii_lowercase();
    if t.len() >= 5 && t.ends_with('s') && !t.ends_with("ss") {
        t.pop();
    }
    t
}

fn stem_query_term(term: &str) -> String {
    let t = stem_plural_only(term);
    stem_derivational_suffix(&t).unwrap_or(t)
}

fn is_camel_boundary(bytes: &[u8], i: usize) -> bool {
    if i == 0 || !bytes[i].is_ascii_uppercase() {
        return false;
    }
    let prev_lower = bytes[i - 1].is_ascii_lowercase() || bytes[i - 1].is_ascii_digit();
    let next_lower =
        bytes.get(i + 1).is_some_and(u8::is_ascii_lowercase) && bytes[i - 1].is_ascii_uppercase();
    prev_lower || next_lower
}

fn normalize_camel_case_lower(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.char_indices() {
        if is_camel_boundary(bytes, i) {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn is_for_stopword(w: &str) -> bool {
    matches!(
        w.to_ascii_lowercase().as_str(),
        "how"
            | "the"
            | "and"
            | "are"
            | "for"
            | "with"
            | "from"
            | "into"
            | "what"
            | "when"
            | "where"
            | "which"
            | "that"
            | "this"
            | "does"
            | "can"
            | "via"
            | "use"
            | "used"
            | "using"
    )
}

fn contains_term_aware(haystack: &str, needle: &str) -> bool {
    if needle.len() >= 4 {
        return haystack.contains(needle);
    }
    haystack
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|tok| tok == needle || (needle.len() == 3 && tok.starts_with(needle)))
}

/// Score a symbol against a single search term across name, scope, and file path.
fn matches_term_or_stem(haystack: &str, t: &str, stem: &str) -> bool {
    contains_term_aware(haystack, t) || contains_term_aware(haystack, stem)
}

fn score_single_term(
    lower_name: &str,
    norm_name: &str,
    lower_scope: Option<&str>,
    lower_path: &str,
    term: &str,
) -> (f64, bool) {
    let t = term.to_ascii_lowercase();
    let stem = stem_query_term(&t);
    let name_hit = if lower_name == t || lower_name == stem {
        30.0
    } else if matches_term_or_stem(lower_name, &t, &stem)
        || matches_term_or_stem(norm_name, &t, &stem)
    {
        12.0
    } else {
        0.0
    };
    let scope_hit = if lower_scope.is_some_and(|sc| matches_term_or_stem(sc, &t, &stem)) {
        6.0
    } else {
        0.0
    };
    let path_hit = if !lower_path.is_empty() && matches_term_or_stem(lower_path, &t, &stem) {
        4.0
    } else {
        0.0
    };
    (
        name_hit + scope_hit + path_hit,
        name_hit > 0.0 || scope_hit > 0.0,
    )
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
            let st = stem_plural_only(&low);
            (low, st)
        })
        .collect();
    for r in &ingest.references {
        let sym_idx = r.from_symbol as usize;
        if sym_idx >= masks.len() {
            continue;
        }
        let callee = interner.resolve(&r.callee_name).to_ascii_lowercase();
        let qual = r
            .qualifier
            .map(|q| interner.resolve(&q).to_ascii_lowercase());
        for (i, (low, st)) in stems.iter().enumerate() {
            let hit = callee.contains(low)
                || callee.contains(st)
                || qual
                    .as_deref()
                    .is_some_and(|q| q.contains(low) || q.contains(st));
            if hit {
                masks[sym_idx] |= 1u32 << i;
            }
        }
    }
    masks
}

fn compute_term_idf_weights(
    ingest: &IngestResult,
    interner: &Interner,
    raw_terms: &[&str],
) -> Vec<f64> {
    if raw_terms.is_empty() {
        return Vec::new();
    }
    let total = ingest
        .symbols
        .iter()
        .filter(|s| s.is_code_symbol())
        .count()
        .max(1) as f64;
    raw_terms
        .iter()
        .map(|&term| {
            let df = ingest
                .symbols
                .iter()
                .filter(|s| s.is_code_symbol() && symbol_matches_term(s, ingest, interner, term))
                .count() as f64;
            1.0 + ((total / (df + 1.0)).ln().max(0.0) * 0.5)
        })
        .collect()
}

struct SymbolMatchCtx<'a> {
    name: &'a str,
    scope: Option<&'a str>,
    file_path: &'a str,
    body_mask: u32,
    terms: &'a [&'a str],
    idf_weights: &'a [f64],
    rank: f64,
}

fn compute_match_synergy(direct_hits: usize, body_only_hits: usize, name_hits: usize) -> f64 {
    let eff_matches = direct_hits as f64 + (body_only_hits as f64) * 0.45;
    eff_matches.powi(3) * (1.0 + (name_hits.saturating_sub(1) as f64) * 0.75)
}

fn score_symbol_match_with_idf(ctx: &SymbolMatchCtx<'_>) -> f64 {
    let lower_name = ctx.name.to_ascii_lowercase();
    let norm_name = normalize_camel_case_lower(ctx.name);
    let lower_scope = ctx.scope.map(normalize_camel_case_lower);
    let lower_path = ctx.file_path.to_ascii_lowercase();
    let mut direct_hits = 0usize;
    let mut body_only_hits = 0usize;
    let mut name_hits = 0usize;
    let mut max_raw = 0.0f64;
    let mut weighted_sum = 0.0f64;
    for (i, &t) in ctx.terms.iter().enumerate() {
        let (base, is_name_hit) = score_single_term(
            &lower_name,
            &norm_name,
            lower_scope.as_deref(),
            &lower_path,
            t,
        );
        name_hits += usize::from(is_name_hit);
        let body = if (ctx.body_mask & (1u32 << i.min(31))) != 0 {
            6.0
        } else {
            0.0
        };
        let raw = base + body;
        if raw > 0.0 {
            direct_hits += usize::from(base > 0.0);
            body_only_hits += usize::from(base == 0.0);
            max_raw = max_raw.max(raw);
            let idf = ctx.idf_weights.get(i).copied().unwrap_or(1.0);
            weighted_sum += raw * idf;
        }
    }
    if max_raw <= 6.0 {
        return 0.0;
    }
    let synergy = compute_match_synergy(direct_hits, body_only_hits, name_hits);
    let query_wants_test = ctx.terms.iter().any(|t| {
        let l = t.to_ascii_lowercase();
        l.contains("test") || l.contains("spec")
    });
    let is_test = crate::gate::affected::is_test_or_bench_path(ctx.file_path)
        || lower_name.starts_with("test_");
    let non_test = if is_test && !query_wants_test {
        0.05
    } else {
        1.0
    };
    weighted_sum * synergy * non_test * (1.0 + ctx.rank * 10.0)
}

fn score_candidate_symbol(
    sym: &crate::domain::Symbol,
    ingest: &IngestResult,
    interner: &Interner,
    ranks: &[f64],
    body_masks: &[u32],
    raw_terms: &[&str],
    idf_weights: &[f64],
) -> f64 {
    use crate::domain::CorpusView;
    if !sym.is_code_symbol() {
        return 0.0;
    }
    let view = (ingest, interner);
    let file_path = view.sym_path(sym);
    let kind_factor = if matches!(sym.kind, SymKind::Var | SymKind::Field) {
        0.2
    } else {
        sym.lang.semantics().retrieval_weight()
    };
    let ctx = SymbolMatchCtx {
        name: view.sym_name(sym),
        scope: view.sym_scope(sym),
        file_path,
        body_mask: body_masks.get(sym.id as usize).copied().unwrap_or(0),
        terms: raw_terms,
        idf_weights,
        rank: ranks.get(sym.id as usize).copied().unwrap_or(0.0),
    };
    score_symbol_match_with_idf(&ctx) * kind_factor
}

fn extract_normalized_sig(
    sym: &crate::domain::Symbol,
    bytes: &[u8],
    fallback_name: &str,
) -> String {
    sym.extract_signature(bytes, fallback_name)
}

fn read_file_cached<'a>(
    root: &str,
    rel_path: &str,
    file_id: u32,
    cache: &'a mut std::collections::HashMap<u32, Vec<u8>>,
) -> &'a [u8] {
    cache
        .entry(file_id)
        .or_insert_with(|| crate::domain::read_rel_bytes(root, rel_path).unwrap_or_default())
}

#[derive(Default, Clone, Copy)]
pub struct ForTaskOptions<'a> {
    pub root: &'a str,
    pub graph: Option<&'a Graph>,
    pub budget_tokens: Option<usize>,
    pub detail: Option<usize>,
    pub adaptive: bool,
}

/// Parse `--sections=`: a closed, comma-separated, order-insensitive set of `lego` and `compose`.
pub fn parse_for_sections(raw: &str) -> Result<Vec<&'static str>, String> {
    let mut out: Vec<&'static str> = Vec::new();
    for part in raw.split(',').map(str::trim) {
        let name = match part {
            "lego" => "lego",
            "compose" => "compose",
            "" => return Err(
                "ripwire-rs: --sections= names an empty section; use lego, compose, or lego,compose"
                    .to_string(),
            ),
            other => {
                return Err(format!(
                    "ripwire-rs: --sections={raw}: unknown section `{other}`; use lego, compose, or lego,compose"
                ));
            }
        };
        if out.contains(&name) {
            return Err(format!(
                "ripwire-rs: --sections={raw}: `{name}` named twice"
            ));
        }
        out.push(name);
    }
    Ok(out)
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
    view: &dyn crate::domain::CorpusView<'_>,
    is_tested: bool,
    opts: ForTaskOptions<'_>,
    file_cache: &mut std::collections::HashMap<u32, Vec<u8>>,
) -> Value {
    let name = view.sym_name(sym);
    let path = view.sym_path(sym);
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
        "tested": u8::from(is_tested),
        "score": (score * 100.0).round() / 100.0
    });
    if idx < detail_count
        && let Some(body) = sym.source_text(bytes)
    {
        obj["body"] = Value::String(body.into_owned());
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

fn symbol_matches_term(
    sym: &crate::domain::Symbol,
    ingest: &IngestResult,
    interner: &Interner,
    term: &str,
) -> bool {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let lower_t = term.to_ascii_lowercase();
    let stem = stem_query_term(&lower_t);
    let name = view.sym_name(sym).to_ascii_lowercase();
    let path = view.sym_path(sym).to_ascii_lowercase();
    contains_term_aware(&name, &lower_t)
        || contains_term_aware(&name, &stem)
        || contains_term_aware(&path, &lower_t)
        || contains_term_aware(&path, &stem)
}

fn diversify_top_candidates<'a>(
    scored: &mut [(f64, &'a crate::domain::Symbol)],
    raw_terms: &[&str],
    ingest: &'a IngestResult,
    interner: &'a Interner,
) {
    if raw_terms.len() < 3 || scored.len() < 6 {
        return;
    }
    let mut insert_pos = 2usize;
    for &term in raw_terms.iter().take(8) {
        if insert_pos >= 8 || insert_pos >= scored.len() {
            break;
        }
        let covered = scored[..insert_pos + 2]
            .iter()
            .any(|(_, s)| symbol_matches_term(s, ingest, interner, term));
        if covered {
            continue;
        }
        if let Some(rel_idx) = scored[insert_pos..].iter().take(60).position(|(_, s)| {
            !matches!(s.kind, SymKind::Var | SymKind::Field)
                && symbol_matches_term(s, ingest, interner, term)
        }) {
            let found_idx = insert_pos + rel_idx;
            scored[insert_pos..=found_idx].rotate_right(1);
            insert_pos += 1;
        }
    }
}

pub fn call_for_task_with_options(
    ingest: &IngestResult,
    interner: &Interner,
    ranks: &[f64],
    task: &str,
    opts: ForTaskOptions<'_>,
) -> Value {
    let all_terms: Vec<&str> = task
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() >= 2)
        .collect();
    let filtered_terms: Vec<&str> = all_terms
        .iter()
        .copied()
        .filter(|w| !is_for_stopword(w))
        .collect();
    let raw_terms = if filtered_terms.is_empty() {
        all_terms
    } else {
        filtered_terms
    };

    let body_masks = build_body_term_masks(ingest, interner, &raw_terms);
    let idf_weights = compute_term_idf_weights(ingest, interner, &raw_terms);
    let mut scored = Vec::new();
    for sym in &ingest.symbols {
        let score = score_candidate_symbol(
            sym,
            ingest,
            interner,
            ranks,
            &body_masks,
            &raw_terms,
            &idf_weights,
        );
        if score > 0.0 {
            scored.push((score, sym));
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    diversify_top_candidates(&mut scored, &raw_terms, ingest, interner);
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
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let tested_bitmap = opts.graph.map(|g| view.compute_test_reachability(g));
    let symbols_json: Vec<Value> = scored[..limit]
        .iter()
        .enumerate()
        .map(|(idx, (score, sym))| {
            let is_tested = tested_bitmap
                .as_ref()
                .and_then(|bm| bm.get(sym.id as usize).copied())
                .unwrap_or_else(|| view.is_test_symbol(sym));
            build_for_symbol_entry(idx, *score, sym, &view, is_tested, opts, &mut file_cache)
        })
        .collect();
    let (route, confidence, margin_pct, coverage) =
        compute_for_route_metadata(task, &raw_terms, &scored, interner);
    let lego = collect_for_lego(ingest, interner, &scored[..limit]);
    let compose = collect_for_compose(ingest, interner, &scored[..limit]);
    let hops = collect_for_hops(ingest, interner, opts.graph, &scored[..limit]);
    let tree = collect_for_tree_rows(ingest, interner, &scored, limit);
    let docs = collect_for_doc_mentions(ingest, interner, &raw_terms);
    let sig_bytes: usize = symbols_json
        .iter()
        .map(|s| {
            s["sig"].as_str().map_or(0, str::len)
                + s["body"].as_str().map_or(0, str::len)
                + s["path"].as_str().map_or(0, str::len)
                + 64
        })
        .sum();
    let est_tokens = ((sig_bytes + 320) / 4).max(1) as u64;
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
        "lego": lego,
        "compose": compose,
        "hops": hops,
        "tree": tree,
        "docs": docs,
        "adaptive": adaptive_info.map(|(k, t, g)| json!({ "kept": k, "total": t, "gap": g }))
    })
}

/// Interfaces that are ranked, or declared in a file holding a ranked symbol, and have implementors.
fn collect_for_lego(
    ingest: &IngestResult,
    interner: &Interner,
    ranked: &[(f64, &crate::domain::Symbol)],
) -> Vec<Value> {
    let ranked_files: std::collections::HashSet<u32> =
        ranked.iter().map(|(_, s)| s.file_id).collect();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for sym in &ingest.symbols {
        if sym.kind != SymKind::Interface || !ranked_files.contains(&sym.file_id) {
            continue;
        }
        let name = interner.resolve(&sym.name);
        if !seen.insert(name) {
            continue;
        }
        let lego = call_lego(ingest, interner, name);
        if lego["count"].as_u64().unwrap_or(0) == 0 {
            continue;
        }
        let path = ingest
            .files
            .get(sym.file_id as usize)
            .map_or("", |f| interner.resolve(&f.path));
        out.push(json!({
            "name": name,
            "path": path,
            "line": sym.line,
            "implementors": lego["implementors"]
        }));
    }
    out
}

fn collect_for_compose(
    ingest: &IngestResult,
    interner: &Interner,
    ranked: &[(f64, &crate::domain::Symbol)],
) -> Vec<Value> {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let ranked_files: std::collections::HashSet<u32> =
        ranked.iter().map(|(_, s)| s.file_id).collect();
    let mut out = Vec::new();
    for sym in &ingest.symbols {
        if sym.kind != SymKind::Field || !ranked_files.contains(&sym.file_id) {
            continue;
        }
        let Some(owner) = view.sym_scope(sym) else {
            continue;
        };
        out.push(json!({
            "name": view.sym_name(sym),
            "owner": owner,
            "rel": "uses",
        }));
        if out.len() >= 32 {
            break;
        }
    }
    out
}

fn collect_for_hops(
    ingest: &IngestResult,
    interner: &Interner,
    graph_opt: Option<&Graph>,
    ranked: &[(f64, &crate::domain::Symbol)],
) -> Vec<Value> {
    use crate::domain::CorpusView;
    let Some(graph) = graph_opt else {
        return Vec::new();
    };
    let view = (ingest, interner);
    let mut hops = Vec::new();
    for &(_, sym) in ranked.iter().take(6) {
        let callees: Vec<Value> = graph
            .callees(sym.id)
            .filter_map(|(cid, _)| ingest.sym(cid))
            .map(|csym| {
                json!({
                    "name": view.sym_name(csym),
                    "line": csym.line,
                })
            })
            .collect();
        if callees.is_empty() {
            continue;
        }
        hops.push(json!({
            "name": view.sym_name(sym),
            "path": view.sym_path(sym),
            "line": sym.line,
            "total": callees.len(),
            "calls": callees.into_iter().take(16).collect::<Vec<_>>(),
        }));
    }
    hops
}

fn collect_for_tree_rows(
    ingest: &IngestResult,
    interner: &Interner,
    scored: &[(f64, &crate::domain::Symbol)],
    sig_limit: usize,
) -> Vec<Value> {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let shown_files: std::collections::HashSet<u32> =
        scored[..sig_limit].iter().map(|(_, s)| s.file_id).collect();
    let mut seen_tree_files = std::collections::HashSet::new();
    let mut rows = Vec::new();
    for &(_, sym) in scored.iter().skip(sig_limit) {
        if rows.len() >= 12 {
            break;
        }
        if !shown_files.contains(&sym.file_id) && seen_tree_files.insert(sym.file_id) {
            rows.push(json!({
                "path": view.sym_path(sym),
                "sym": view.sym_name(sym),
                "kind": sym.kind.tag(),
            }));
        }
    }
    rows
}

fn collect_for_doc_mentions(
    ingest: &IngestResult,
    interner: &Interner,
    raw_terms: &[&str],
) -> Vec<Value> {
    let mut doc_hits: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for sym in &ingest.symbols {
        if sym.kind != SymKind::Section || !sym.lang.semantics().is_prose_document() {
            continue;
        }
        let sec_lower = interner.resolve(&sym.name).to_ascii_lowercase();
        let hits = raw_terms
            .iter()
            .filter(|&&t| {
                let low = t.to_ascii_lowercase();
                contains_term_aware(&sec_lower, &low)
            })
            .count();
        if hits > 0 {
            let p = ingest.sym_file_path(sym, interner);
            *doc_hits.entry(p).or_default() += hits;
        }
    }
    let mut docs: Vec<(&str, usize)> = doc_hits.into_iter().collect();
    docs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    docs.into_iter()
        .take(4)
        .map(|(p, m)| json!({ "path": p, "mentions": m }))
        .collect()
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
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let Some(sym) = view.find_symbol_with_hint(symbol_name, path_hint) else {
        return json!({ "error": format!("symbol '{symbol_name}' not found") });
    };
    let file_path = view.sym_path(sym);
    if file_path.is_empty() {
        return json!({ "error": "file info missing" });
    }
    let bytes = match view.read_sym_bytes(root, sym) {
        Ok(b) => b,
        Err(e) => return json!({ "error": format!("cannot read file '{file_path}': {e}") }),
    };
    let body_str = sym.source_text(&bytes).unwrap_or_default();
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
