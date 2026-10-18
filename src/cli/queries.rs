pub use super::arch::*;
pub use super::workspace::*;

use super::args::TopRetrievalArgs;
use crate::domain::{IngestResult, Interner, NodeId};
use crate::graph::Graph;
use crate::rank::PageRankResult;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavMode {
    Callers,
    Callees,
    Impact,
    Uses,
    FindSymbol,
}

impl NavMode {
    #[must_use]
    pub const fn cli_flag(self) -> &'static str {
        match self {
            Self::Callers => "--callers",
            Self::Callees => "--callees",
            Self::Impact => "--impact",
            Self::Uses => "--uses",
            Self::FindSymbol => "--find-symbol",
        }
    }

    #[must_use]
    pub fn from_batch_verb(verb: &str) -> Option<Self> {
        match verb {
            "callers" => Some(Self::Callers),
            "callees" => Some(Self::Callees),
            "impact" => Some(Self::Impact),
            "uses" => Some(Self::Uses),
            "find_symbol" | "find-symbol" => Some(Self::FindSymbol),
            _ => None,
        }
    }
}

impl std::fmt::Display for NavMode {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.cli_flag())
    }
}

impl std::str::FromStr for NavMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_batch_verb(s).ok_or_else(|| format!("unknown navigation mode: '{s}'"))
    }
}
pub struct NavContext<'a> {
    pub ingest: &'a IngestResult,
    pub graph: &'a Graph,
    pub ranks: &'a [f64],
    pub interner: &'a Interner,
}

impl<'a> crate::domain::CorpusView<'a> for NavContext<'a> {
    #[inline]
    fn ingest(&self) -> &'a IngestResult {
        self.ingest
    }

    #[inline]
    fn interner(&self) -> &'a Interner {
        self.interner
    }
}

pub fn serialize_1hop_mode<W: Write>(
    root: &str,
    sym: &str,
    mode: NavMode,
    nav: &crate::navigate::NavNeighborhood,
    out: &mut W,
) -> io::Result<()> {
    match mode {
        NavMode::Callers => crate::navigate::serialize_callers_with_gauge_xml(
            root,
            sym,
            &nav.callers,
            &nav.gauge_attrs,
            out,
        ),
        NavMode::Callees => crate::navigate::serialize_callees_with_gauge_xml(
            root,
            sym,
            &nav.callees,
            &nav.gauge_attrs,
            out,
        ),
        NavMode::FindSymbol => crate::navigate::serialize_find_symbol_xml(nav, out),
        _ => Ok(()),
    }
}

pub fn execute_nav_symbol_query<W: Write>(
    sym: &str,
    mode: NavMode,
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let ctx = NavContext {
        ingest,
        graph,
        ranks,
        interner,
    };
    execute_nav_symbol_query_in_root(".", sym, mode, &ctx, out)
}

pub fn execute_nav_symbol_query_in_root<W: Write>(
    root: &str,
    sym: &str,
    mode: NavMode,
    ctx: &NavContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    let ids = crate::navigate::find_symbols_by_name(sym, ctx.ingest, ctx.interner);
    if ids.is_empty() {
        let flag = mode.cli_flag();
        if let Some(near) = crate::navigate::find_nearest_symbol_name(sym, ctx.ingest, ctx.interner)
        {
            return Err(io::Error::other(format!(
                "ripwire-rs: {flag} symbol not found: {sym} (did you mean '{near}'?)"
            )));
        }
        return Err(io::Error::other(format!(
            "ripwire-rs: {flag} symbol not found: {sym}"
        )));
    }
    let gauge_attrs = ctx.graph.xml_gauge_attrs();
    match mode {
        NavMode::Uses => crate::navigate::serialize_uses_with_gauge_xml(
            root,
            sym,
            ctx.ingest,
            ctx.interner,
            &gauge_attrs,
            out,
        ),
        NavMode::Impact => {
            use crate::domain::CorpusView;
            let impact = crate::navigate::transitive_impact_multi(
                &ids,
                ctx.ingest,
                ctx.graph,
                ctx.ranks,
                ctx.interner,
                None,
            );
            let importers = (ctx.ingest, ctx.interner).resolve_symbol_importers(&ids);
            let pr_iters = ctx
                .graph
                .page_rank(&crate::rank::PageRankConfig::default())
                .iterations;
            crate::navigate::serialize_impact_with_importers_xml(
                root,
                sym,
                &impact,
                importers,
                &gauge_attrs,
                pr_iters,
                out,
            )
        }
        NavMode::Callers | NavMode::Callees | NavMode::FindSymbol => {
            if let Some(nav) = crate::navigate::navigate_1hop_multi(
                &ids,
                ctx.ingest,
                ctx.graph,
                ctx.ranks,
                ctx.interner,
            ) {
                serialize_1hop_mode(root, sym, mode, &nav, out)?;
            }
            Ok(())
        }
    }
}

pub fn execute_connect_query<W: Write>(
    syms: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    execute_connect_query_in_root(".", syms, ingest, graph, interner, out)
}

pub fn execute_connect_query_in_root<W: Write>(
    root: &str,
    syms: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    execute_connect_query_with_radius(root, syms, 6, ingest, graph, interner, out)
}

pub fn execute_connect_query_with_radius<W: Write>(
    root: &str,
    syms: &str,
    max_radius: u32,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let sym_list: Vec<String> = syms
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    for s in &sym_list {
        if crate::navigate::find_symbols_by_name(s, ingest, interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire-rs: --connect symbol not found: {s}"
            )));
        }
    }
    let mut val = crate::mcp::graph_tools::call_connect_with_radius(
        ingest, graph, interner, &sym_list, max_radius,
    );
    val["root"] = serde_json::Value::String(root.to_string());
    crate::serialize::serialize_connect_xml(&val, out)
}

pub fn split_endpoint_pair(pair: &str) -> (&str, &str) {
    if let Some((from, to)) = pair.split_once(',') {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.split_once("->") {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.rsplit_once(':')
        && !from.ends_with(':')
    {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.split_once(':') {
        (from.trim(), to.trim())
    } else {
        (pair.trim(), "")
    }
}

pub fn execute_path_query<W: Write>(
    pair: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (from, to) = split_endpoint_pair(pair);
    if crate::navigate::find_symbols_by_name(from, ingest, interner).is_empty() {
        return Err(io::Error::other(format!(
            "ripwire-rs: --path endpoint not found: {from}"
        )));
    }
    if crate::navigate::find_symbols_by_name(to, ingest, interner).is_empty() {
        return Err(io::Error::other(format!(
            "ripwire-rs: --path endpoint not found: {to}"
        )));
    }
    let val = crate::mcp::graph_tools::call_path_between(ingest, graph, interner, from, to);
    crate::serialize::serialize_path_between_xml(&val, out)
}

pub fn extract_symbol_body<'a>(
    sym: &'a crate::domain::Symbol,
    ingest: &'a IngestResult,
    interner: &'a Interner,
    root_path: &Path,
) -> Option<(&'a crate::domain::Symbol, &'a str, &'a str, String)> {
    let name = &interner[sym.name];
    let rel_path = ingest.sym_file_path(sym, interner);
    let bytes = std::fs::read(root_path.join(rel_path)).ok()?;
    let body = sym.source_text(&bytes)?.into_owned();
    Some((sym, name, rel_path, body))
}

pub fn skeletonize_body(body: &str) -> String {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut elided = false;
    for line in body.lines() {
        let opens = line.chars().filter(|&c| c == '{').count() as i32;
        let closes = line.chars().filter(|&c| c == '}').count() as i32;
        if depth <= 1 {
            out.push(line.to_string());
            elided = false;
        } else if !elided {
            out.push("    ...".to_string());
            elided = true;
        }
        depth += opens - closes;
        if depth <= 1 && closes > 0 && elided {
            out.push(line.to_string());
            elided = false;
        }
    }
    out.join("\n")
}

fn collect_expanded_symbol<'a>(
    id: NodeId,
    is_outline: bool,
    ingest: &'a IngestResult,
    interner: &'a Interner,
    root_path: &Path,
) -> Option<(&'a crate::domain::Symbol, &'a str, &'a str, String)> {
    let (sym, name, rel_path, body) =
        extract_symbol_body(&ingest.symbols[id as usize], ingest, interner, root_path)?;
    let final_body = if is_outline {
        skeletonize_body(&body)
    } else {
        body
    };
    Some((sym, name, rel_path, final_body))
}

pub fn execute_expand_or_outline_query<W: Write>(
    root: &str,
    syms_spec: &str,
    is_outline: bool,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let sym_names: Vec<&str> = syms_spec
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut expanded_items = Vec::new();
    let root_path = Path::new(root);

    for sym_name in sym_names {
        let ids = crate::navigate::find_symbols_by_name(sym_name, ingest, interner);
        if ids.is_empty() {
            let flag = if is_outline { "--outline" } else { "--expand" };
            return Err(io::Error::other(format!(
                "ripwire-rs: {flag} symbol not found: {sym_name}"
            )));
        }
        for &id in &ids {
            if let Some(item) = collect_expanded_symbol(id, is_outline, ingest, interner, root_path)
            {
                expanded_items.push(item);
            }
        }
    }

    let items_slice: Vec<(&crate::domain::Symbol, &str, &str, &str)> = expanded_items
        .iter()
        .map(|(s, n, p, b)| (*s, *n, *p, b.as_str()))
        .collect();

    crate::serialize::serialize_expand_xml(root, &items_slice, out)
}

pub fn execute_expand_query<W: Write>(
    root: &str,
    syms_spec: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    execute_expand_or_outline_query(root, syms_spec, false, ingest, interner, out)
}

pub fn execute_pack_top_n_query<W: Write>(
    root: &str,
    n: usize,
    ingest: &IngestResult,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (file_order, buckets) = crate::serialize::bucket_symbols_by_rank(
        &ingest.symbols,
        ingest.files.len(),
        &pr.ranks,
        Some(n),
    );
    let mut bodies = Vec::new();
    let mut count = 0;

    for f in file_order {
        if count >= n {
            break;
        }
        let rel_path = ingest.file_path(f, interner);
        if let Ok(bytes) = crate::domain::read_rel_bytes(root, rel_path) {
            let content = String::from_utf8_lossy(&bytes).to_string();
            bodies.push((rel_path, content));
            count += buckets[f as usize].len();
        }
    }

    let bodies_slice: Vec<(&str, &str)> = bodies.iter().map(|(p, c)| (*p, c.as_str())).collect();
    crate::serialize::serialize_pack_top_n_xml(root, &bodies_slice, out)
}

pub fn execute_tree_query<W: Write>(
    root: &str,
    ingest: &IngestResult,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (file_order, buckets) = crate::serialize::bucket_symbols_by_rank(
        &ingest.symbols,
        ingest.files.len(),
        &pr.ranks,
        None,
    );

    let mut file_symbols = Vec::new();
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    for f in file_order {
        let rel_path = view.file_path(f);
        let bucket = &buckets[f as usize];
        let mut top3 = Vec::new();
        for &sym_id in bucket.iter().take(3) {
            let sym = &ingest[sym_id];
            top3.push((sym.kind.tag(), view.sym_name(sym)));
        }
        file_symbols.push(crate::serialize::TreeFileSymbols {
            path: rel_path,
            total_symbols: bucket.len(),
            top_symbols: top3,
        });
    }

    use crate::serialize::XmlReport;
    let files_unlisted = ingest.files.len().saturating_sub(file_symbols.len());
    crate::serialize::query_xml::TreeReport {
        root,
        file_symbols: &file_symbols,
        files_unlisted,
        pr_iters: pr.iterations,
    }
    .render_envelope(out)
}

pub fn deduplicate_batch_legend(raw: &[u8], legend_emitted: &mut bool) -> Vec<u8> {
    let text = String::from_utf8_lossy(raw);
    if !text.contains("<!--") {
        return raw.to_vec();
    }
    if !*legend_emitted {
        *legend_emitted = true;
        return raw.to_vec();
    }
    let mut out = String::with_capacity(text.len());
    let mut s: &str = &text;
    while let Some(start) = s.find("<!--") {
        out.push_str(&s[..start]);
        if let Some(end) = s[start..].find("-->") {
            s = &s[start + end + 3..];
        } else {
            break;
        }
    }
    out.push_str(s.trim_start());
    let trimmed = out.trim_end();
    format!(
        "{trimmed}\n<about legend=\"ref\" dictv=\"{}\"/>\n",
        crate::serialize::LEGEND_DICT_VERSION
    )
    .into_bytes()
}

pub fn run_batch_subquery<W: Write>(
    line: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let mut first = false;
    run_batch_subquery_dedup(line, ingest, graph, pr, interner, &mut first, out)
}

fn run_batch_subquery_dedup<W: Write>(
    line: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    legend_emitted: &mut bool,
    out: &mut W,
) -> io::Result<()> {
    let Some((verb, arg)) = line.split_once(':') else {
        return Ok(());
    };
    let a = arg.trim();
    let mut sub_buf = Vec::new();
    let verb_trimmed = verb.trim();
    if let Ok(mode) = verb_trimmed.parse::<NavMode>() {
        let _ = execute_nav_symbol_query(a, mode, ingest, graph, &pr.ranks, interner, &mut sub_buf);
    } else if verb_trimmed == "grep" {
        let val = crate::mcp::retrieval::call_grep(ingest, interner, a, None);
        let _ = crate::serialize::serialize_grep_xml(&val, &mut sub_buf);
    } else if verb_trimmed == "for" {
        let val = crate::mcp::retrieval::call_for_task_with_options(
            ingest,
            interner,
            &pr.ranks,
            a,
            crate::mcp::retrieval::ForTaskOptions {
                root: ".",
                graph: Some(graph),
                ..Default::default()
            },
        );
        let _ = crate::serialize::serialize_for_xml(&val, &mut sub_buf);
    }
    let final_bytes = deduplicate_batch_legend(&sub_buf, legend_emitted);
    writeln!(out, "  <result query=\"{line}\">")?;
    out.write_all(&final_bytes)?;
    writeln!(out, "  </result>")
}

pub fn execute_batch_content<W: Write>(
    root: &str,
    content: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let lines: Vec<&str> = content
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .take(16)
        .collect();

    writeln!(
        out,
        "<batch schema=\"ripwire-rs.batch/v1\" root=\"{root}\" dictv=\"{}\" count=\"{}\">",
        crate::serialize::LEGEND_DICT_VERSION,
        lines.len()
    )?;
    let mut legend_emitted = false;
    for line in lines {
        run_batch_subquery_dedup(line, ingest, graph, pr, interner, &mut legend_emitted, out)?;
    }
    writeln!(out, "</batch>")
}
pub fn execute_batch_file<W: Write>(
    root: &str,
    file_path: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let content = if file_path == "-" {
        let mut buf = String::new();
        io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
        buf
    } else {
        std::fs::read_to_string(file_path)?
    };
    execute_batch_content(root, &content, ingest, graph, pr, interner, out)
}

fn execute_task_or_signatures<W: Write>(
    ret: &TopRetrievalArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    if let Some(task) = ret.task_for.as_deref() {
        let detail = if ret.signatures_only {
            None
        } else if ret.auto_bodies && ret.detail.is_none() {
            Some(6)
        } else {
            ret.detail
        };
        let mut val = crate::mcp::retrieval::call_for_task_with_options(
            ingest,
            interner,
            &pr.ranks,
            task,
            crate::mcp::retrieval::ForTaskOptions {
                root,
                graph: Some(graph),
                budget_tokens: None,
                detail,
                adaptive: ret.adaptive,
            },
        );
        if let Some(in_pat) = ret.in_scope.as_deref()
            && let Some(arr) = val
                .get_mut("symbols")
                .and_then(serde_json::Value::as_array_mut)
        {
            arr.retain(|s| s["path"].as_str().is_some_and(|p| p.contains(in_pat)));
        }
        if let Some(raw) = ret.sections.as_deref() {
            let sections =
                crate::mcp::retrieval::parse_for_sections(raw).map_err(io::Error::other)?;
            val["sections"] = serde_json::json!(sections);
        }
        crate::serialize::serialize_for_xml(&val, out)?;
        return Ok(true);
    }
    if ret.pack_signatures {
        writeln!(out, "<ctx schema=\"ripwire-rs.pack-signatures/v1\">")?;
        let opts = crate::serialize::SerializeOptions {
            root: root.to_string(),
            ..Default::default()
        };
        crate::serialize::serialize_xml(ingest, graph, pr, interner, out, &opts)?;
        writeln!(out, "</ctx>")?;
        return Ok(true);
    }
    Ok(false)
}

fn recommend_verb_for_task(task: &str) -> (&'static str, &'static str) {
    let lower = task.to_ascii_lowercase();
    if lower.contains("caller") || lower.contains("who calls") {
        ("--callers", "1-hop callers of a named symbol")
    } else if lower.contains("blast") || lower.contains("impact") || lower.contains("safe") {
        ("--impact", "transitive caller blast radius")
    } else if lower.contains("trace") || lower.contains("crash") || lower.contains("error") {
        ("--from-trace", "map stack trace onto indexed symbols")
    } else if lower.contains("duplicate") || lower.contains("clone") {
        ("--clones", "AST near-miss code clone detector")
    } else if lower.contains("test") || lower.contains("gate") {
        (
            "--test-gate",
            "affected test suite and untested blast radius",
        )
    } else {
        ("--for", "task-lens BM25 + PageRank symbol retrieval")
    }
}

fn execute_pattern_or_match<W: Write>(
    ret: &TopRetrievalArgs,
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    if let Some(pat) = ret.pattern.as_deref() {
        let val = crate::mcp::retrieval::call_pattern_search(
            root,
            ingest,
            interner,
            pat,
            ret.grep_in.as_deref(),
        );
        crate::serialize::serialize_grep_xml(&val, out)?;
    } else if let Some(q) = ret.ast_match.as_deref() {
        let val = crate::mcp::retrieval::call_ast_match(root, ingest, interner, q)
            .map_err(io::Error::other)?;
        crate::serialize::serialize_grep_xml(&val, out)?;
    } else if let Some(pat) = ret.regex.as_deref() {
        let before = ret.grep_before.or(ret.grep_context).unwrap_or(0);
        let after = ret.grep_after.or(ret.grep_context).unwrap_or(0);
        let val = crate::mcp::retrieval::call_grep_with_context(
            root,
            ingest,
            interner,
            pat,
            ret.grep_in.as_deref(),
            before,
            after,
        );
        crate::serialize::serialize_grep_xml(&val, out)?;
    } else if let Some(task) = ret.help_task.as_deref() {
        let (verb, reason) = recommend_verb_for_task(task);
        let t_esc = crate::serialize::escape::escape_xml_str(task);
        writeln!(
            out,
            "<help-task schema=\"ripwire-rs.help-task/v1\" task=\"{t_esc}\" recommended=\"{verb}\" why=\"{reason}\"/>"
        )?;
    } else if let Some(handle) = ret.refetch.as_deref() {
        execute_expand_or_outline_query(root, handle, false, ingest, interner, out)?;
    }
    Ok(())
}

fn execute_advanced_retrieval<W: Write>(
    ret: &TopRetrievalArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    if let Some(tr) = ret.from_trace.as_deref() {
        let trace_text = if tr == "-" {
            let mut buf = String::new();
            let _ = io::Read::read_to_string(&mut io::stdin(), &mut buf);
            buf
        } else {
            std::fs::read_to_string(tr).unwrap_or_else(|_| tr.to_string())
        };
        let val = crate::mcp::graph_tools::call_from_trace(ingest, interner, &trace_text);
        crate::serialize::serialize_from_trace_xml(&val, out)
    } else if let Some(at_spec) = ret.at.as_deref() {
        let (file, line_str) = at_spec.split_once(':').unwrap_or((at_spec, "1"));
        let line: u32 = line_str.parse().unwrap_or(1);
        crate::navigate::serialize_at_xml(file, line, ingest, interner, out)
    } else if let Some(claim) = ret.verify.as_deref() {
        crate::navigate::evaluate_verify_claim(claim, ingest, graph, interner, out)
    } else if let Some(task) = ret.pack_task.as_deref() {
        crate::navigate::execute_pack_task(task, root, ingest, interner, out)
    } else if let Some(expr) = ret.graph_query.as_deref() {
        crate::navigate::execute_graph_query_expr(expr, root, ingest, graph, interner, out)
    } else {
        execute_pattern_or_match(ret, root, ingest, interner, out)
    }
}

fn execute_grep_cli_dispatch<W: Write>(
    ret: &TopRetrievalArgs,
    pat: &str,
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let before = ret.grep_before.or(ret.grep_context).unwrap_or(0);
    let after = ret.grep_after.or(ret.grep_context).unwrap_or(0);
    let path_filter = ret
        .grep_in
        .as_deref()
        .or(ret.grep_scope.as_deref())
        .or(ret.in_scope.as_deref());
    let mut val = crate::mcp::retrieval::call_grep_with_context(
        root,
        ingest,
        interner,
        pat,
        path_filter,
        before,
        after,
    );
    if let Some(arr) = val
        .get_mut("hits")
        .and_then(serde_json::Value::as_array_mut)
    {
        if let Some(and_p) = ret.and_pat.as_deref() {
            arr.retain(|h| h["text"].as_str().is_some_and(|t| t.contains(and_p)));
        }
        if let Some(not_p) = ret.not_pat.as_deref() {
            arr.retain(|h| !h["text"].as_str().is_some_and(|t| t.contains(not_p)));
        }
    }
    crate::serialize::serialize_grep_xml(&val, out)
}

pub fn execute_retrieval_dispatch<W: Write>(
    ret: &TopRetrievalArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    if execute_task_or_signatures(ret, root, ingest, graph, pr, interner, out)? {
        return Ok(());
    }
    if let Some(pat) = ret.grep.as_deref() {
        execute_grep_cli_dispatch(ret, pat, root, ingest, interner, out)
    } else if let Some(syms) = ret.connect.as_deref() {
        execute_connect_query_with_radius(
            root,
            syms,
            ret.connect_radius,
            ingest,
            graph,
            interner,
            out,
        )
    } else if let Some(pair) = ret.path_between.as_deref() {
        execute_path_query(pair, ingest, graph, interner, out)
    } else if let Some(ex) = ret.exemplar.as_deref() {
        let val = crate::mcp::retrieval::call_exemplar_in_root(
            root,
            ingest,
            interner,
            &pr.ranks,
            ex,
            Some(graph),
        );
        if val.get("error").is_some() {
            return Err(io::Error::other(
                "ripwire-rs: --exemplar: no candidate found",
            ));
        }
        crate::serialize::serialize_exemplar_xml(root, &val, out)
    } else if let Some(iface) = ret.lego.as_deref() {
        if crate::navigate::find_symbols_by_name(iface, ingest, interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire-rs: --lego type not found: {iface}"
            )));
        }
        use crate::serialize::JsonValueExt;
        let mut val = crate::mcp::retrieval::call_lego(ingest, interner, iface);
        val.attach_gauges(graph);
        crate::serialize::serialize_lego_xml(root, &val, out)
    } else if let Some(rec) = ret.recall.as_deref() {
        let val =
            crate::mcp::retrieval::call_memory_recall_in_root(root, ingest, interner, rec, None);
        crate::serialize::serialize_recall_xml(root, rec, &val, out)
    } else {
        execute_advanced_retrieval(ret, root, ingest, graph, interner, out)
    }
}
