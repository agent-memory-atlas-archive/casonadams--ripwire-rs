//! `--connect A,B,...`: the minimal subgraph relating a set of symbols.
use super::common::mcp::mcp_tool;
use super::common::retrieval::report_query;
use super::{Plugin, Run};
use crate::query::commands::{CommandDescriptor, QueryContext, render_xml};
use crate::query::graph as graph_tools;
use crate::serialize::JsonValueExt;
use clap::Args;
use std::io::{self, Write};

/// `--connect` and `--connect-radius`.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct ConnectArgs {
    /// Minimal connecting subgraph relating symbols (A,B,...)
    #[arg(long, value_name = "A,B")]
    pub connect: Option<String>,
    /// Connecting radius for --connect
    #[arg(long = "connect-radius", value_name = "N")]
    pub connect_radius: Option<u32>,
}

pub fn execute<W: Write>(query: &QueryContext<'_>, syms: &str, radius: Option<u32>, out: &mut W) -> io::Result<()> {
    crate::query::graph::execute_connect_query_with_radius(
        query.root,
        syms,
        radius.unwrap_or(graph_tools::DEFAULT_CONNECT_RADIUS),
        query.ingest,
        query.graph,
        query.interner,
        out,
    )
}

fn run(run: &Run<'_>, args: &ConnectArgs) -> io::Result<()> {
    let syms = args.connect.as_deref().unwrap_or_default();
    report_query(run, |query, out| execute(query, syms, args.connect_radius, out))
}

pub static CONNECT: Plugin<ConnectArgs> = Plugin {
    name: "connect",
    options: &[],
    selected: |args| args.connect.is_some(),
    run,
    check: None,
    tools: TOOLS,
};

static TOOLS: &[&CommandDescriptor] = &[mcp_tool!(
    "connect",
    "When a task touches 2..16 named symbols, returns the minimal subgraph relating them",
    &[
        ("symbols", "string[]", "List of symbol names"),
        ("radius", "integer", "Connection search radius"),
    ],
    &["symbols"],
    |s, a| {
        let symbols: Vec<&str> = a["symbols"]
            .as_array()
            .ok_or("symbols must be an array")?
            .iter()
            .map(|name| name.as_str().ok_or("symbol names must be strings"))
            .collect::<Result<_, _>>()?;
        let request = graph_tools::ConnectRequest {
            root: s.root,
            symbols: &symbols,
            radius: a
                .opt_usize("radius")
                .unwrap_or(graph_tools::DEFAULT_CONNECT_RADIUS as usize)
                as u32,
        };
        render_xml(|out| {
            graph_tools::execute_connect(&request, s.ingest, s.graph, s.interner, out)
        })
    }
)];
