//! `--around SYM`: ego-graph of call neighbours around one symbol.
use super::{Plugin, Run};
use crate::commands::context::ExecutionContext;
use crate::domain::{IngestResult, Interner, NodeId};
use crate::graph::Graph;
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct AroundArgs {
    /// Ego-graph around a symbol
    #[arg(long, value_name = "SYM")]
    pub around: Option<String>,
    /// Depth for --around ego-graph (default: 1)
    #[arg(long = "around-depth", default_value_t = 1)]
    pub around_depth: u32,
    /// Fanout limit for --around ego-graph (default: 32)
    #[arg(long = "around-fanout", default_value_t = 32)]
    pub around_fanout: usize,
}

/// Configuration for `--around` ego-graph queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AroundConfig {
    pub depth: u32,
    pub fanout: usize,
}

fn enqueue_around_neighbors(
    curr: NodeId,
    next_d: u32,
    fanout: usize,
    graph: &Graph,
    visited: &mut std::collections::HashSet<NodeId>,
    queue: &mut std::collections::VecDeque<(NodeId, u32)>,
) -> usize {
    let mut cut = 0usize;
    for (idx, (callee_id, _)) in graph.callees(curr).enumerate() {
        if idx < fanout {
            if visited.insert(callee_id) {
                queue.push_back((callee_id, next_d));
            }
        } else {
            cut += 1;
        }
    }
    for (idx, (caller_id, _)) in graph.callers(curr).enumerate() {
        if idx < fanout {
            if visited.insert(caller_id) {
                queue.push_back((caller_id, next_d));
            }
        } else {
            cut += 1;
        }
    }
    cut
}

fn collect_around_callees<'a>(
    curr: NodeId,
    fanout: usize,
    visited: &std::collections::HashSet<NodeId>,
    ingest: &'a IngestResult,
    graph: &Graph,
    interner: &'a Interner,
) -> Vec<&'a str> {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let mut callees = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (callee_id, _) in graph.callees(curr) {
        if visited.contains(&callee_id)
            && let Some(cs) = ingest.sym(callee_id)
        {
            let cn = view.sym_name(cs);
            if seen.insert(cn) {
                callees.push(cn);
                if callees.len() >= fanout {
                    break;
                }
            }
        }
    }
    callees
}

pub fn execute_around_query<W: Write>(
    root: &str,
    sym_name: &str,
    cfg: AroundConfig,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    use crate::domain::CorpusView;
    use crate::navigate::SymbolLookup;
    let view = (ingest, interner);
    let ids = view.find_symbols(sym_name);
    if ids.is_empty() {
        return Err(io::Error::other(format!(
            "ripwire-rs: --around symbol not found: {sym_name}"
        )));
    }
    let sym_id = ids
        .iter()
        .copied()
        .find(|&id| ingest.sym(id).is_some_and(|s| view.is_production_symbol(s)))
        .unwrap_or(ids[0]);
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    visited.insert(sym_id);
    queue.push_back((sym_id, 0u32));

    let mut ordered_nodes = Vec::new();
    let mut fanout_cut = 0usize;
    while let Some((curr, curr_d)) = queue.pop_front() {
        ordered_nodes.push((curr, curr_d));
        if curr_d < cfg.depth {
            fanout_cut += enqueue_around_neighbors(
                curr,
                curr_d + 1,
                cfg.fanout,
                graph,
                &mut visited,
                &mut queue,
            );
        }
    }

    let mut rows = Vec::with_capacity(ordered_nodes.len());
    for (curr, hop) in ordered_nodes {
        if let Some(s) = ingest.sym(curr) {
            rows.push(crate::serialize::AroundSymbolRow {
                name: view.sym_name(s),
                scope: view.sym_scope(s),
                kind: s.kind.tag(),
                path: view.sym_path(s),
                hop,
                callees: collect_around_callees(
                    curr, cfg.fanout, &visited, ingest, graph, interner,
                ),
            });
        }
    }

    crate::serialize::serialize_around_rows_xml(
        root, sym_name, cfg.depth, cfg.fanout, fanout_cut, &rows, out,
    )
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    args: &AroundArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let cfg = AroundConfig {
        depth: args.around_depth,
        fanout: args.around_fanout,
    };
    execute_around_query(
        ctx.root,
        args.around.as_deref().unwrap_or_default(),
        cfg,
        ctx.ingest,
        ctx.graph,
        ctx.interner,
        out,
    )
}

fn run(run: &Run<'_>, args: &AroundArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub static AROUND: Plugin<AroundArgs> = Plugin {
    name: "around",
    options: &[],
    selected: |args| args.around.is_some(),
    run,
    check: None,
    tools: &[],
};
