//! `--at FILE:LINE`: the innermost symbol chain enclosing a source location.
use super::common::retrieval::report_query;
use super::{Plugin, Run};
use crate::query::commands::QueryContext;
use clap::Args;
use std::io::{self, Write};

/// `--at`.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct AtArgs {
    /// Resolve innermost symbol chain enclosing FILE:LINE
    #[arg(long = "at", value_name = "FILE:LINE")]
    pub at: Option<String>,
}

pub fn execute<W: Write>(query: &QueryContext<'_>, at_spec: &str, out: &mut W) -> io::Result<()> {
    let (file, line_str) = at_spec.split_once(':').unwrap_or((at_spec, "1"));
    let line: u32 = line_str.parse().unwrap_or(1);
    crate::navigate::serialize_at_xml(file, line, query.ingest, query.interner, out)
}

fn run(run: &Run<'_>, args: &AtArgs) -> io::Result<()> {
    let at_spec = args.at.as_deref().unwrap_or_default();
    report_query(run, |query, out| execute(query, at_spec, out))
}

pub static AT: Plugin<AtArgs> = Plugin {
    name: "at",
    options: &[],
    selected: |args| args.at.is_some(),
    run,
    check: None,
    tools: &[],
};
