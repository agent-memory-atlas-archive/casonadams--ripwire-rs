//! `--lego TYPE`: one interface and every type that implements it.
use super::common::mcp::mcp_tool;
use super::SharedOption;
use super::plugin::report_command;
use crate::query::commands::{QueryContext, render_query_value};
use crate::query::retrieval;
use crate::serialize::{JsonValueExt, PageRequest, Paged};
use std::io::{self, Write};

pub fn execute<W: Write>(query: &QueryContext<'_>, iface: &str, out: &mut W) -> io::Result<()> {
    let mut val = retrieval::call_lego(query.ingest, query.interner, iface);
    if let Some(error) = val.get("error").and_then(serde_json::Value::as_str) {
        return Err(io::Error::other(error.to_owned()));
    }
    val.attach_gauges(query.graph);
    crate::serialize::serialize_lego_xml(query.root, &val, out)
}

/// Keep the implementors inside the request's window; the interface records the full count.
fn window_implementors(val: &mut serde_json::Value, page: PageRequest) {
    let Some(all) = val["implementors"].as_array() else {
        return;
    };
    let total = all.len();
    let kept = Paged::new(all, page, |imp| imp.to_string().len())
        .rows()
        .to_vec();
    val["implementors_total"] = serde_json::Value::from(total);
    val["implementors"] = serde_json::Value::Array(kept);
}

report_command! {
    name: "lego",
    args: LegoArgs,
    static: LEGO,
    options: [SharedOption::McpPaged],
    tools: [mcp_tool!(
        "lego",
        "Interface-to-impls view: method contract plus EVERY implementor",
        &[
            ("interface", "string", "Interface, trait or base class name"),
        ],
        &["interface"],
        |s, a| {
            let mut val = retrieval::call_lego(s.ingest, s.interner, a.req_str("interface")?);
            val.attach_gauges(s.graph);
            window_implementors(&mut val, a.page_request());
            render_query_value(val, a, |val, out| {
                crate::serialize::serialize_lego_xml(s.root, val, out)
            })
        }
    )],
    run: query(execute),
    /// Show one interface and every type implementing it
    #[arg(long, value_name = "TYPE")]
    lego: Option<String>,
}
