use super::args::TopRetrievalArgs;
use crate::query::commands::QueryContext;
use crate::query::graph::{execute_connect_query_with_radius, execute_path_query};
use std::io::{self, Write};

impl TopRetrievalArgs {
    pub fn for_options<'a>(
        &'a self,
        ctx: &QueryContext<'a>,
        budget_tokens: Option<usize>,
    ) -> crate::query::retrieval::ForTaskOptions<'a> {
        crate::query::retrieval::ForTaskOptions {
            root: ctx.root,
            at: ctx.at,
            graph: Some(ctx.graph),
            sources: Some(ctx.sources),
            budget_tokens,
            in_scope: self.in_scope.as_deref(),
            sections: self.sections.as_deref(),
            signatures_only: self.signatures_only,
            auto_bodies: self.auto_bodies,
            detail: self.detail,
            adaptive: self.adaptive,
        }
    }
}

fn execute_task_or_signatures<W: Write>(
    ret: &TopRetrievalArgs,
    ctx: &QueryContext<'_>,
    budget_tokens: Option<usize>,
    out: &mut W,
) -> io::Result<bool> {
    let QueryContext {
        root,
        ingest,
        graph,
        pr,
        interner,
        ..
    } = *ctx;
    if let Some(task) = ret.task_for.as_deref() {
        let val = crate::query::retrieval::call_for_task_with_options(
            ingest,
            interner,
            &pr.ranks,
            task,
            ret.for_options(ctx, budget_tokens),
        );
        if let Some(error) = val.get("error").and_then(serde_json::Value::as_str) {
            return Err(io::Error::other(error.to_owned()));
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
    ctx: &QueryContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    if let Some(pat) = ret.pattern.as_deref() {
        let val = crate::query::retrieval::call_pattern_search(
            ctx.sources,
            ctx.ingest,
            ctx.interner,
            pat,
            ret.in_scope.as_deref(),
        );
        crate::serialize::serialize_grep_xml(&val, out)?;
    } else if let Some(q) = ret.ast_match.as_deref() {
        let val = crate::query::retrieval::call_ast_match(ctx.sources, ctx.ingest, ctx.interner, q)
            .map_err(io::Error::other)?;
        crate::serialize::serialize_grep_xml(&val, out)?;
    } else if let Some(task) = ret.help_task.as_deref() {
        super::task_route::write_help_task_xml(task, ctx.root, ctx.ingest, ctx.interner, out)?;
    } else if let Some(handle) = ret.refetch.as_deref() {
        let request = crate::query::detail::BodyRequest {
            root: ctx.root,
            symbols: handle,
            path_hint: None,
            start_line: None,
            end_line: None,
            outline: false,
            compress: false,
        };
        crate::query::detail::execute_body_query(
            &request,
            ctx.ingest,
            ctx.interner,
            ctx.sources,
            out,
        )?;
    }
    Ok(())
}

fn execute_advanced_retrieval<W: Write>(
    ret: &TopRetrievalArgs,
    ctx: &QueryContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    if let Some(tr) = ret.from_trace.as_deref() {
        let trace_text = if tr == "-" {
            let mut buf = String::new();
            let _ = io::Read::read_to_string(&mut io::stdin(), &mut buf);
            buf
        } else if tr.contains('\n') || tr.contains(':') {
            tr.to_string()
        } else if let Ok(cow) = ctx.sources.read(tr) {
            String::from_utf8_lossy(&cow).into_owned()
        } else {
            tr.to_string()
        };
        let val = crate::query::graph::call_from_trace(ctx.ingest, ctx.interner, &trace_text);
        crate::serialize::serialize_from_trace_xml(&val, out)
    } else if let Some(at_spec) = ret.at.as_deref() {
        let (file, line_str) = at_spec.split_once(':').unwrap_or((at_spec, "1"));
        let line: u32 = line_str.parse().unwrap_or(1);
        crate::navigate::serialize_at_xml(file, line, ctx.ingest, ctx.interner, out)
    } else if let Some(claim) = ret.verify.as_deref() {
        let v_ctx = crate::navigate::VerifyContext::from(ctx);
        crate::navigate::evaluate_verify_claim(claim, &v_ctx, out)
    } else if let Some(task) = ret.pack_task.as_deref() {
        crate::navigate::execute_pack_task(task, ctx, out)
    } else if let Some(expr) = ret.graph_query.as_deref() {
        crate::navigate::execute_graph_query_expr(
            expr,
            ctx.root,
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            out,
        )
    } else {
        execute_pattern_or_match(ret, ctx, out)
    }
}

pub fn execute_grep_cli_dispatch<W: Write>(
    ret: &TopRetrievalArgs,
    verb: &str,
    pat: &str,
    ctx: &super::dispatch::ExecutionContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    let sources = crate::query::FileSources {
        root: std::path::Path::new(ctx.root),
    };
    let corpus = crate::crawler::walk_corpus(ctx.root, &crate::crawler::CrawlerOptions::default());
    let req = crate::query::retrieval::GrepRequest {
        corpus_paths: &corpus.rel_paths,
        pruned_dirs: corpus.pruned_dirs,
        limit: ctx.cli.modifiers.limit,
        offset: ctx.cli.modifiers.offset.unwrap_or(0),
        verb,
        path_filter: ret.in_scope.as_deref(),
        any_tier: crate::query::grep::parse_any_tier(ret.grep_in.as_deref())?,
        before: ret.grep_before.or(ret.grep_context).unwrap_or(0),
        after: ret.grep_after.or(ret.grep_context).unwrap_or(0),
        and_terms: &ret.and_pat,
        not_terms: &ret.not_pat,
        file_scope: ret.grep_scope.as_deref() == Some("file"),
        graph: Some(ctx.graph),
        ..crate::query::retrieval::GrepRequest::new(ctx.root, pat, &sources)
    };
    let val = crate::query::retrieval::call_grep(ctx.ingest, ctx.interner, &req);
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
        .or_else(|| ret.regex.as_deref().map(|p| ("regex", p)))
        .filter(|_| ret.task_for.is_none());
    match target {
        Some((verb, pat)) => execute_grep_cli_dispatch(ret, verb, pat, ctx, out),
        None => {
            let sources = crate::query::FileSources {
                root: std::path::Path::new(ctx.root),
            };
            let at = crate::git::history::compute_git_at_stamp(ctx.root);
            let query = QueryContext {
                root: ctx.root,
                ingest: ctx.ingest,
                graph: ctx.graph,
                pr: ctx.pr,
                interner: ctx.interner,
                sources: &sources,
                commits: None,
                at: at.as_deref(),
            };
            execute_retrieval_dispatch(
                ret,
                &query,
                ctx.cli.token_budget.or(ctx.cli.max_tokens),
                out,
            )
        }
    }
}

pub fn execute_retrieval_dispatch<W: Write>(
    ret: &TopRetrievalArgs,
    ctx: &QueryContext<'_>,
    budget_tokens: Option<usize>,
    out: &mut W,
) -> io::Result<()> {
    let QueryContext {
        root,
        ingest,
        graph,
        pr,
        interner,
        sources,
        ..
    } = *ctx;
    if execute_task_or_signatures(ret, ctx, budget_tokens, out)? {
        return Ok(());
    }
    if let Some(syms) = ret.connect.as_deref() {
        execute_connect_query_with_radius(
            root,
            syms,
            ret.connect_radius
                .unwrap_or(crate::query::graph::DEFAULT_CONNECT_RADIUS),
            ingest,
            graph,
            interner,
            out,
        )
    } else if let Some(pair) = ret.path_between.as_deref() {
        execute_path_query(pair, ingest, graph, interner, out)
    } else if let Some(ex) = ret.exemplar.as_deref() {
        let val = crate::query::retrieval::call_exemplar_in_root(
            sources,
            ingest,
            interner,
            &pr.ranks,
            ex,
            Some(graph),
        );
        if let Some(error) = val.get("error").and_then(serde_json::Value::as_str) {
            return Err(io::Error::other(error.to_owned()));
        }
        crate::serialize::serialize_exemplar_xml(root, &val, out)
    } else if let Some(iface) = ret.lego.as_deref() {
        use crate::serialize::JsonValueExt;
        let mut val = crate::query::retrieval::call_lego(ingest, interner, iface);
        if let Some(error) = val.get("error").and_then(serde_json::Value::as_str) {
            return Err(io::Error::other(error.to_owned()));
        }
        val.attach_gauges(graph);
        crate::serialize::serialize_lego_xml(root, &val, out)
    } else {
        execute_advanced_retrieval(ret, ctx, out)
    }
}
