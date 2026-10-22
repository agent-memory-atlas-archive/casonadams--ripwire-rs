//! `--ensemble`: four orthogonal evidence families joined and ranked.
use super::{Plugin, Run, SharedOption};
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct EnsembleArgs {
    /// Four orthogonal evidence families joined, ranked by distinct families fired
    #[arg(long = "ensemble")]
    pub ensemble: bool,
}

pub static ENSEMBLE: Plugin<EnsembleArgs> = Plugin {
    name: "ensemble",
    options: &[SharedOption::Paged],
    selected: |a| a.ensemble,
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, args: &EnsembleArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    _args: &EnsembleArgs,
    out: &mut XmlWriter<'_, impl std::io::Write>,
) -> io::Result<()> {
    let rep = crate::metrics::compute_ensemble(
        ctx.ingest,
        ctx.graph,
        ctx.interner,
        ctx.cli.page_request(),
    );
    ctx.write_report(&rep, out)?;
    Ok(())
}
