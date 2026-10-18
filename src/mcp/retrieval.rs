use crate::domain::{IngestResult, Interner, RefRole, SymKind};
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

#[cfg(test)]
fn score_symbol_match_in_file(
    name: &str,
    scope: Option<&str>,
    file_path: &str,
    body_mask: u32,
    terms: &[&str],
    rank: f64,
) -> f64 {
    score_symbol_match_with_idf(&SymbolMatchCtx {
        name,
        scope,
        file_path,
        body_mask,
        terms,
        idf_weights: &[],
        rank,
    })
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
    let kind_factor = if matches!(sym.kind, SymKind::Var | SymKind::Field) {
        0.2
    } else {
        1.0
    };
    let ctx = SymbolMatchCtx {
        name: view.sym_name(sym),
        scope: view.sym_scope(sym),
        file_path: view.sym_path(sym),
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
                "ripwire: --sections= names an empty section; use lego, compose, or lego,compose"
                    .to_string(),
            ),
            other => {
                return Err(format!(
                    "ripwire: --sections={raw}: unknown section `{other}`; use lego, compose, or lego,compose"
                ));
            }
        };
        if out.contains(&name) {
            return Err(format!("ripwire: --sections={raw}: `{name}` named twice"));
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
        if sym.kind != SymKind::Section || sym.lang != crate::domain::Lang::Markdown {
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

    use crate::domain::CorpusView;
    let view = (ingest, interner);
    for &sym in candidates_pool {
        let is_non_test = !crate::gate::affected::is_test_path(view.sym_path(sym));
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
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let name = view.sym_name(sym);
    let file_path = view.sym_path(sym);
    let body = view.read_sym_source(root, sym);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_for_sections_is_a_closed_set() {
        assert_eq!(parse_for_sections("lego").unwrap(), vec!["lego"]);
        assert_eq!(
            parse_for_sections("compose, lego").unwrap(),
            vec!["compose", "lego"]
        );
        assert!(
            parse_for_sections("lego,lego")
                .unwrap_err()
                .contains("named twice")
        );
        assert!(
            parse_for_sections("legos")
                .unwrap_err()
                .contains("unknown section")
        );
        assert!(parse_for_sections("").is_err());
    }

    #[test]
    fn test_for_lego_section_stub_by_default_full_on_request() {
        let mut val = json!({
            "task": "parse flags",
            "symbols": [],
            "lego": [{
                "name": "VerbGroup",
                "path": "src/cli/args.rs",
                "line": 30,
                "implementors": [
                    { "name": "TopNavArgs", "path": "src/cli/args.rs", "line": 122 },
                    { "name": "TopDetailArgs", "path": "src/cli/args.rs", "line": 142 }
                ]
            }]
        });
        let render = |v: &Value| {
            let mut out = Vec::new();
            crate::serialize::serialize_for_xml(v, &mut out).unwrap();
            String::from_utf8(out).unwrap()
        };
        let stub = render(&val);
        assert!(stub.contains("<lego total=\"1\" shown=\"0\" capped=\"1\" next=\"--for=&apos;parse flags&apos; --sections=lego\"/>"));
        assert!(!stub.contains("<iface "));

        val["sections"] = json!(["lego"]);
        let full = render(&val);
        assert!(full.contains("<lego><iface n=\"VerbGroup\" p=\"src/cli/args.rs:30\" implementors=\"2\"><impl n=\"TopNavArgs\" p=\"src/cli/args.rs:122\"/>"));
        assert!(!full.contains("<lego total="));

        val["lego"] = json!([]);
        assert!(!render(&val).contains("<lego"));
        let mut cval = json!({
            "task": "parse flags",
            "symbols": [],
            "compose": [{ "name": "graph", "owner": "NavContext", "rel": "uses" }],
            "hops": [{ "name": "run", "path": "src/main.rs", "line": 10, "total": 1, "calls": [{ "name": "init", "line": 2 }] }]
        });
        let cstub = render(&cval);
        assert!(cstub.contains("<compose total=\"1\" shown=\"0\" capped=\"1\""));
        assert!(cstub.contains("<hops shown=\"1\" total=\"1\" capped=\"0\"><h l=\"10\" p=\"src/main.rs\" n=\"run\"><calls total=\"1\"><c n=\"init\" l=\"2\"/></calls></h></hops>"));
        cval["sections"] = json!(["compose"]);
        let cfull = render(&cval);
        assert!(cfull.contains(
            "<compose><field name=\"graph\" owner=\"NavContext\" rel=\"uses\"/></compose>"
        ));
    }
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

        for i in 1..6 {
            ingest.symbols.push(crate::domain::Symbol::new(
                i,
                interner.intern(&format!("session_secret_{i}")),
                interner.intern(&format!("c_{i}")),
                SymKind::Function,
                crate::domain::Lang::Rust,
                0,
                i + 1,
            ));
        }
        ingest.symbols.push(crate::domain::Symbol::new(
            6,
            interner.intern("lsp_manager"),
            interner.intern("c_lsp"),
            SymKind::Struct,
            crate::domain::Lang::Rust,
            0,
            10,
        ));
        let ranks = vec![1.0; ingest.symbols.len()];
        let div_res = call_for_task(
            &ingest,
            &interner,
            &ranks,
            "lsp manager session secret",
            None,
            None,
            false,
        );
        let top_names: Vec<&str> = div_res["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .take(5)
            .filter_map(|s| s["name"].as_str())
            .collect();
        assert!(top_names.contains(&"lsp_manager"));
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
        let ts_code = b"export const materializeUrl = async (rawPath: string): Promise<string> => {\n  const x = rawPath.trim();\n  return x;\n};\n";
        let mut ingest = crate::ingest::ingest_in_memory_files(
            &[
                ("src/lib.rs", crate::domain::Lang::Rust, code),
                ("src/url.ts", crate::domain::Lang::TypeScript, ts_code),
            ],
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

        let arrow_sym = ingest
            .symbols
            .iter()
            .find(|s| interner.resolve(&s.name) == "materializeUrl")
            .expect("materializeUrl");
        assert!(arrow_sym.has_body());
        assert!(
            !arrow_sym
                .extract_signature(ts_code, "materializeUrl")
                .contains("rawPath.trim()")
        );

        let next_id = ingest.symbols.len() as u32;
        ingest.symbols.push(
            crate::domain::Symbol::new(
                next_id,
                interner.intern("count"),
                interner.intern("src/lib.rs::Widget::count"),
                SymKind::Field,
                crate::domain::Lang::Rust,
                0,
                1,
            )
            .with_scope(interner.intern("Widget")),
        );
        ingest.references.push(crate::domain::Reference::new(
            0,
            interner.intern("build"),
            0,
            1,
            crate::domain::Lang::Rust,
            RefRole::Call,
        ));
        let graph = crate::graph::Graph::build(&ingest, &interner);
        let pr = graph.page_rank(&crate::rank::PageRankConfig::default());
        let res = call_for_task_with_options(
            &ingest,
            &interner,
            &pr.ranks,
            "Widget build",
            ForTaskOptions {
                root: ".",
                graph: Some(&graph),
                budget_tokens: None,
                detail: Some(1),
                adaptive: false,
            },
        );
        assert!(!res["compose"].as_array().unwrap().is_empty());
        assert!(!res["hops"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_camel_case_and_derivational_stem_scoring() {
        assert_eq!(
            normalize_camel_case_lower("discoverAndLoadASTNodes"),
            "discover_and_load_ast_nodes"
        );
        assert_eq!(stem_query_term("discovery"), "discover");
        assert_eq!(stem_query_term("validation"), "valid");
        assert_eq!(stem_query_term("resolution"), "resolv");

        let s_discover = score_symbol_match_in_file(
            "discoverAndLoadASTNodes",
            None,
            "packages/core/src/ast/loader.ts",
            0b11111,
            &["ast", "node", "cache", "tree", "discovery"],
            0.001,
        );
        let s_reload = score_symbol_match_in_file(
            "reloadTrees",
            None,
            "packages/core/src/controllers/tree-controller.ts",
            0b11111,
            &["ast", "node", "cache", "tree", "discovery"],
            0.001,
        );
        assert!(s_discover > s_reload);
    }

    #[test]
    fn test_call_lego_includes_out_of_line_methods_for_unique_class() {
        let interner = Interner::new();
        let mut ingest = IngestResult::new();
        ingest.files.push(crate::domain::FileInfo {
            id: 0,
            path: interner.intern("src/infra/Diagnostics.h"),
            lang: crate::domain::Lang::Cpp,
            line_count: 50,
            byte_size: 500,
        });
        ingest.files.push(crate::domain::FileInfo {
            id: 1,
            path: interner.intern("src/infra/diagnostics.cpp"),
            lang: crate::domain::Lang::Cpp,
            line_count: 50,
            byte_size: 500,
        });
        let cls_spur = interner.intern("ConsoleLog");
        ingest.symbols.push(crate::domain::Symbol::new(
            0,
            cls_spur,
            interner.intern("src/infra/Diagnostics.h::ConsoleLog"),
            SymKind::Class,
            crate::domain::Lang::Cpp,
            0,
            10,
        ));
        ingest.symbols.push(
            crate::domain::Symbol::new(
                1,
                interner.intern("handlePanic"),
                interner.intern("src/infra/Diagnostics.h::ConsoleLog::handlePanic"),
                SymKind::Function,
                crate::domain::Lang::Cpp,
                0,
                15,
            )
            .with_scope(cls_spur),
        );
        ingest.symbols.push(
            crate::domain::Symbol::new(
                2,
                interner.intern("handleAssert"),
                interner.intern("src/infra/diagnostics.cpp::ConsoleLog::handleAssert"),
                SymKind::Method,
                crate::domain::Lang::Cpp,
                1,
                25,
            )
            .with_scope(cls_spur),
        );
        let lego = call_lego(&ingest, &interner, "ConsoleLog");
        let methods = lego["methods"].as_array().unwrap();
        assert_eq!(methods.len(), 2);
    }
}
