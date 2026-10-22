//! `--at FILE:LINE`: the innermost symbol chain enclosing a source location.
use super::plugin::report_command;
use crate::query::commands::QueryContext;
use std::io::{self, Write};

pub fn execute<W: Write>(query: &QueryContext<'_>, at_spec: &str, out: &mut W) -> io::Result<()> {
    let (file, line_str) = at_spec.split_once(':').unwrap_or((at_spec, "1"));
    let line: u32 = line_str.parse().unwrap_or(1);
    crate::navigate::serialize_at_xml(file, line, query.ingest, query.interner, out)
}

report_command! {
    name: "at",
    args: AtArgs,
    static: AT,
    run: query(execute),
    /// Resolve innermost symbol chain enclosing FILE:LINE
    #[arg(long = "at", value_name = "FILE:LINE")]
    at: Option<String>,
}
