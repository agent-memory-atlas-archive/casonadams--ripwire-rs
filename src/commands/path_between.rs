//! `--path-between A:B`: the shortest directed call path between two symbols.
use super::common::mcp::mcp_tool;
use super::common::retrieval::report_query;
use super::{Plugin, Run};
use crate::query::commands::{CommandDescriptor, QueryContext, render_xml};
use crate::query::graph as graph_tools;
use crate::serialize::JsonValueExt;
use clap::Args;
use std::io::{self, Write};

/// `--path-between`.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct PathBetweenArgs {
    /// Shortest directed call path between two symbols (A:B or A,B)
    #[arg(long = "path-between", visible_alias = "path", value_name = "A:B")]
    pub path_between: Option<String>,
}

pub fn execute<W: Write>(query: &QueryContext<'_>, pair: &str, out: &mut W) -> io::Result<()> {
    crate::query::graph::execute_path_query(pair, query.ingest, query.graph, query.interner, out)
}

fn run(run: &Run<'_>, args: &PathBetweenArgs) -> io::Result<()> {
    let pair = args.path_between.as_deref().unwrap_or_default();
    report_query(run, |query, out| execute(query, pair, out))
}

pub static PATH_BETWEEN: Plugin<PathBetweenArgs> = Plugin {
    name: "path-between",
    options: &[],
    selected: |args| args.path_between.is_some(),
    run,
    check: None,
    tools: TOOLS,
};

static TOOLS: &[&CommandDescriptor] = &[mcp_tool!(
    "path_between",
    "Does A REACH B, and HOW? Shortest directed call path between two symbols",
    &[
        ("from", "string", "Source symbol name"),
        ("to", "string", "Target symbol name"),
    ],
    &["from", "to"],
    |s, a| {
        let request = graph_tools::PathRequest {
            from: a.req_str("from")?,
            to: a.req_str("to")?,
        };
        render_xml(|out| {
            graph_tools::execute_path_between(&request, s.ingest, s.graph, s.interner, out)
        })
    }
)];
