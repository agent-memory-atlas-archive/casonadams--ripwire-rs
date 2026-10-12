use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::rank::PageRankResult;
use crate::serialize::SerializeOptions;
use std::io::{self, Write};

/// Arguments for streaming ranked XML call graph generation.
pub struct RankedStreamArgs<'a> {
    pub root: &'a str,
    pub rank_by: &'a str,
    pub pack_budget: Option<usize>,
    pub top_k: Option<usize>,
    pub emit_legend: bool,
}

pub fn execute_stream_xml_with<W: Write>(
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let args = RankedStreamArgs {
        root,
        rank_by: "pagerank",
        pack_budget: None,
        top_k: None,
        emit_legend: true,
    };
    execute_stream_xml_ranked(&args, ingest, graph, pr, interner, out)
}

pub fn execute_stream_xml_ranked<W: Write>(
    args: &RankedStreamArgs<'_>,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let budget_bytes = args
        .pack_budget
        .or(Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES));
    let rank_mode = args.rank_by.to_ascii_lowercase();

    let (ranks, rank_lbl) = if rank_mode == "authority" {
        let hits = graph.hits();
        (hits.authority, Some("authority".to_string()))
    } else if rank_mode == "hub" {
        let hits = graph.hits();
        (hits.hub, Some("hub".to_string()))
    } else {
        (pr.ranks.clone(), None)
    };

    let stream_ctx = crate::serialize::StreamXmlContext {
        ingest,
        graph,
        ranks: &ranks,
        pr_iters: pr.iterations,
        pr_converged: pr.converged,
        interner,
    };

    crate::serialize::serialize_xml_with_ranks(
        &stream_ctx,
        out,
        &SerializeOptions {
            root: args.root.to_string(),
            top_k: args.top_k,
            include_pr: true,
            pack_budget_bytes: budget_bytes,
            rank_by: rank_lbl,
            emit_legend: args.emit_legend,
        },
    )
}
