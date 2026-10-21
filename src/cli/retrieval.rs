use super::args::TopRetrievalArgs;
use super::detail::execute_expand_or_outline_query;
use super::queries::{execute_connect_query_with_radius, execute_path_query};
use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::rank::PageRankResult;
use std::io::{self, Write};

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
            ret.in_scope.as_deref(),
        );
        crate::serialize::serialize_grep_xml(&val, out)?;
    } else if let Some(q) = ret.ast_match.as_deref() {
        let val = crate::mcp::retrieval::call_ast_match(root, ingest, interner, q)
            .map_err(io::Error::other)?;
        crate::serialize::serialize_grep_xml(&val, out)?;
    } else if let Some(task) = ret.help_task.as_deref() {
        super::task_route::write_help_task_xml(task, root, ingest, interner, out)?;
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

pub fn execute_grep_cli_dispatch<W: Write>(
    ret: &TopRetrievalArgs,
    verb: &str,
    pat: &str,
    ctx: &super::dispatch::ExecutionContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    let req = crate::mcp::retrieval::GrepRequest {
        verb,
        path_filter: ret.in_scope.as_deref(),
        any_tier: ret.grep_in.as_deref() == Some("any"),
        before: ret.grep_before.or(ret.grep_context).unwrap_or(0),
        after: ret.grep_after.or(ret.grep_context).unwrap_or(0),
        and_terms: &ret.and_pat,
        not_terms: &ret.not_pat,
        file_scope: ret.grep_scope.as_deref() == Some("file"),
        graph: Some(ctx.graph),
        ..crate::mcp::retrieval::GrepRequest::new(ctx.root, pat)
    };
    let mut val = crate::mcp::retrieval::call_grep(ctx.ingest, ctx.interner, &req);
    if let Some(limit) = ctx.cli.modifiers.limit {
        val["limit"] = limit.into();
    }
    if let Some(offset) = ctx.cli.modifiers.offset {
        val["offset"] = offset.into();
    }
    crate::serialize::serialize_grep_xml(&val, out)
}

pub fn execute_retrieval_verb<W: Write>(
    ret: &TopRetrievalArgs,
    ctx: &super::dispatch::ExecutionContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    let target = ret
        .grep
        .as_deref()
        .map(|p| ("grep", p))
        .or_else(|| ret.regex.as_deref().map(|p| ("regex", p)));
    match target {
        Some((verb, pat)) => execute_grep_cli_dispatch(ret, verb, pat, ctx, out),
        None => execute_retrieval_dispatch(
            ret,
            ctx.root,
            ctx.ingest,
            ctx.graph,
            ctx.pr,
            ctx.interner,
            out,
        ),
    }
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
    if let Some(syms) = ret.connect.as_deref() {
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
    } else {
        execute_advanced_retrieval(ret, root, ingest, graph, interner, out)
    }
}
