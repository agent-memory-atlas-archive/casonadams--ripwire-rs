//! `--exemplar KIND`: the single best-in-class existing instance of a symbol kind to imitate.
use super::common::mcp::mcp_tool;
use super::plugin::report_command;
use crate::query::commands::{QueryContext, render_query_value};
use crate::query::retrieval;
use crate::serialize::JsonValueExt;
use std::io::{self, Write};

pub fn execute<W: Write>(query: &QueryContext<'_>, spec: &str, out: &mut W) -> io::Result<()> {
    let val = retrieval::call_exemplar_in_root(
        query.sources,
        query.ingest,
        query.interner,
        &query.pr.ranks,
        spec,
        Some(query.graph),
    );
    if let Some(error) = val.get("error").and_then(serde_json::Value::as_str) {
        return Err(io::Error::other(error.to_owned()));
    }
    crate::serialize::serialize_exemplar_xml(query.root, &val, out)
}

report_command! {
    name: "exemplar",
    args: ExemplarArgs,
    static: EXEMPLAR,
    tools: [mcp_tool!(
        "exemplar",
        "BEFORE writing a function/method/class/struct, get repo's single best-in-class instance to imitate",
        &[
            (
                "role",
                "string",
                "Kind of symbol: fn, method, class, struct, interface"
            ),
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
        &[],
        |s, a| {
            let val = retrieval::call_exemplar_in_root(
                s.sources,
                s.ingest,
                s.interner,
                &s.pr.ranks,
                a.str_any(&["role", "kind"], "fn"),
                Some(s.graph),
            );
            render_query_value(val, a, |val, out| {
                crate::serialize::serialize_exemplar_xml(s.root, val, out)
            })
        }
    )],
    run: query(execute),
    /// Retrieve best-in-class existing instance of code to imitate
    #[arg(long, value_name = "TASK|KIND")]
    exemplar: Option<String>,
}
