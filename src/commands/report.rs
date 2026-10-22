//! `--report`: executive markdown architecture report (always plain markdown).
use super::common::safety::context_of;
use super::{Plugin, Run};
use crate::commands::context::Paginated;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct ReportArgs {
    /// Generate executive markdown architecture report
    #[arg(long = "report")]
    pub report: bool,
}

pub fn render(ctx: &ExecutionContext<'_>, out: &mut XmlWriter<'_, impl Write>) -> io::Result<()> {
    crate::metrics::serialize_executive_report_md(
        &context_of(ctx),
        &ctx.pr.ranks,
        ctx.cli.page_request(),
        out,
    )
}

fn run(run: &Run<'_>, _args: &ReportArgs) -> io::Result<()> {
    run.report_plain(render)
}

pub static REPORT: Plugin<ReportArgs> = Plugin {
    name: "report",
    options: &[],
    selected: |args| args.report,
    run,
    check: None,
    tools: &[],
};
