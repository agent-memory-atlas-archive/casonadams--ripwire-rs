//! `--path-between A:B`: the shortest directed call path between two symbols.
use super::common::mcp::mcp_tool;
use super::plugin::report_command;
use crate::query::commands::{QueryContext, render_xml};
use crate::query::graph as graph_tools;
use crate::serialize::JsonValueExt;
use std::io::{self, Write};

pub fn execute<W: Write>(query: &QueryContext<'_>, pair: &str, out: &mut W) -> io::Result<()> {
    crate::query::graph::execute_path_query(pair, query.ingest, query.graph, query.interner, out)
}

report_command! {
    name: "path-between",
    args: PathBetweenArgs,
    static: PATH_BETWEEN,
    tools: [mcp_tool!(
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
    )],
    run: query(execute),
    /// Shortest directed call path between two symbols (A:B or A,B)
    #[arg(long = "path-between", visible_alias = "path", value_name = "A:B")]
    path_between: Option<String>,
}
