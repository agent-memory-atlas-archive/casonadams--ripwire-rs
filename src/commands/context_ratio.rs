//! `--context-ratio`: share of each unit's context lying outside its own file.
use super::{Plugin, Run, SharedOption};
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct ContextRatioArgs {
    /// Local-reasoning lens measuring share of context outside a unit's file
    #[arg(long = "context-ratio")]
    pub context_ratio: bool,
}

pub static CONTEXT_RATIO: Plugin<ContextRatioArgs> = Plugin {
    name: "context-ratio",
    options: &[SharedOption::Paged],
    selected: |a| a.context_ratio,
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, args: &ContextRatioArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    _args: &ContextRatioArgs,
    out: &mut XmlWriter<'_, impl std::io::Write>,
) -> io::Result<()> {
    let rep = crate::metrics::compute_context_ratio(
        ctx.ingest,
        ctx.graph,
        ctx.interner,
        ctx.cli.page_request(),
    );
    ctx.write_report(&rep, out)?;
    Ok(())
}
