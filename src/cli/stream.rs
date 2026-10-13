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
    pub map_diff: bool,
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
        map_diff: false,
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

    let (ranks, rank_lbl) = if args.map_diff {
        let changed_files = crate::cli::safety::get_working_tree_changed_files(args.root);
        let n = ingest.symbols.len();
        let mut weights = vec![0.3 / (n.max(1) as f64); n];
        for (i, sym) in ingest.symbols.iter().enumerate() {
            let f = interner.resolve(&ingest.files[sym.file_id as usize].path);
            if changed_files.iter().any(|cf| cf == f || cf.ends_with(f)) {
                weights[i] = 0.7;
            }
        }
        let tel = crate::rank::pagerank::biased_teleport(&weights);
        let custom = crate::rank::pagerank::page_rank(
            &graph.in_edges,
            &graph.w_out_deg,
            &tel,
            &crate::rank::pagerank::PageRankConfig::default(),
        );
        (custom.ranks, Some("map-diff".to_string()))
    } else if rank_mode == "authority" {
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
            metrics: false,
        },
    )
}
