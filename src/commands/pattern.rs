//! `--pattern PAT`: code-shaped structural pattern search; honors `--in` and `--limit/--offset`.
use super::common::grep::write_hits;
use super::common::retrieval::report_query;
use super::{Plugin, Run, SharedOption};
use crate::query::commands::QueryContext;
use crate::query::retrieval;
use crate::serialize::PageRequest;
use clap::Args;
use std::io::{self, Write};

/// `--pattern`.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct PatternArgs {
    /// Code-shaped structural pattern search
    #[arg(long = "pattern", value_name = "PAT")]
    pub pattern: Option<String>,
}

pub fn execute<W: Write>(
    query: &QueryContext<'_>,
    pat: &str,
    in_scope: Option<&str>,
    page: PageRequest,
    out: &mut W,
) -> io::Result<()> {
    let val = retrieval::call_pattern_search(query.sources, query.ingest, query.interner, pat, in_scope);
    write_hits(val, page, out)
}

fn run(run: &Run<'_>, args: &PatternArgs) -> io::Result<()> {
    let pat = args.pattern.as_deref().unwrap_or_default();
    let in_scope = run.cli.modifiers.in_scope.as_deref();
    report_query(run, |query, out| execute(query, pat, in_scope, run.page(), out))
}

pub static PATTERN: Plugin<PatternArgs> = Plugin {
    name: "pattern",
    options: &[SharedOption::Paged],
    selected: |args| args.pattern.is_some(),
    run,
    check: None,
    tools: &[],
};
