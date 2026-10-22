//! `--from-trace FILE|TEXT|-`: maps a stack trace or compiler error onto indexed symbols, innermost first.
use super::common::mcp::mcp_tool;
use super::plugin::report_command;
use crate::query::commands::{QueryContext, render_xml};
use crate::query::graph as graph_tools;
use crate::serialize::JsonValueExt;
use std::io::{self, Write};

/// `-` reads stdin, text with a newline or `:` is used as-is, and any other value is read as a file when it exists.
fn trace_text(query: &QueryContext<'_>, tr: &str) -> String {
    if tr == "-" {
        let mut buf = String::new();
        let _ = io::Read::read_to_string(&mut io::stdin(), &mut buf);
        buf
    } else if tr.contains('\n') || tr.contains(':') {
        tr.to_string()
    } else if let Ok(cow) = query.sources.read(tr) {
        String::from_utf8_lossy(&cow).into_owned()
    } else {
        tr.to_string()
    }
}

pub fn execute<W: Write>(query: &QueryContext<'_>, tr: &str, out: &mut W) -> io::Result<()> {
    let text = trace_text(query, tr);
    let val = graph_tools::call_from_trace(query.ingest, query.interner, &text);
    crate::serialize::serialize_from_trace_xml(&val, out)
}

report_command! {
    name: "from-trace",
    args: FromTraceArgs,
    static: FROM_TRACE,
    tools: [mcp_tool!(
        "from_trace",
        "Paste a stack trace / compiler error and get it mapped onto indexed symbols innermost-first",
        &[("trace", "string", "Stack trace text")],
        &["trace"],
        |s, a| {
            let val = graph_tools::call_from_trace(s.ingest, s.interner, a.req_str("trace")?);
            render_xml(|out| crate::serialize::serialize_from_trace_xml(&val, out))
        }
    )],
    run: query(execute),
    /// Map a stack trace, sanitizer report or compiler error onto indexed symbols
    #[arg(long = "from-trace", value_name = "FILE")]
    from_trace: Option<String>,
}
