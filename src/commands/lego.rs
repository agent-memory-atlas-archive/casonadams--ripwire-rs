//! `--lego TYPE`: one interface and every type that implements it.
use super::common::mcp::mcp_tool;
use super::common::retrieval::report_query;
use super::{Plugin, Run};
use crate::query::commands::{CommandDescriptor, QueryContext, render_query_value};
use crate::query::retrieval;
use crate::serialize::JsonValueExt;
use clap::Args;
use std::io::{self, Write};

/// `--lego`.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct LegoArgs {
    /// Show one interface and every type implementing it
    #[arg(long, value_name = "TYPE")]
    pub lego: Option<String>,
}

pub fn execute<W: Write>(query: &QueryContext<'_>, iface: &str, out: &mut W) -> io::Result<()> {
    let mut val = retrieval::call_lego(query.ingest, query.interner, iface);
    if let Some(error) = val.get("error").and_then(serde_json::Value::as_str) {
        return Err(io::Error::other(error.to_owned()));
    }
    val.attach_gauges(query.graph);
    crate::serialize::serialize_lego_xml(query.root, &val, out)
}

fn run(run: &Run<'_>, args: &LegoArgs) -> io::Result<()> {
    let iface = args.lego.as_deref().unwrap_or_default();
    report_query(run, |query, out| execute(query, iface, out))
}

pub static LEGO: Plugin<LegoArgs> = Plugin {
    name: "lego",
    options: &[],
    selected: |args| args.lego.is_some(),
    run,
    check: None,
    tools: TOOLS,
};

static TOOLS: &[&CommandDescriptor] = &[mcp_tool!(
    "lego",
    "Interface-to-impls view: method contract plus EVERY implementor",
    &[
        ("interface", "string", "Interface, trait or base class name"),
        ("limit", "integer", "Maximum report items"),
        ("offset", "integer", "Report item offset"),
    ],
    &["interface"],
    |s, a| {
        let mut val = retrieval::call_lego(s.ingest, s.interner, a.req_str("interface")?);
        val.attach_gauges(s.graph);
        render_query_value(val, a, |val, out| {
            crate::serialize::serialize_lego_xml(s.root, val, out)
        })
    }
)];
