//! `--biggest-first`: functions ranked by readability volume and size, largest first.
use super::{Plugin, Run, SharedOption};
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct BiggestFirstArgs {
    /// Rank functions by readability volume and size, largest first
    #[arg(long = "biggest-first")]
    pub biggest_first: bool,
}

pub static BIGGEST_FIRST: Plugin<BiggestFirstArgs> = Plugin {
    name: "biggest-first",
    options: &[SharedOption::Paged],
    selected: |a| a.biggest_first,
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, args: &BiggestFirstArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    _args: &BiggestFirstArgs,
    out: &mut XmlWriter<'_, impl std::io::Write>,
) -> io::Result<()> {
    crate::metrics::execute_biggest_first_query(
        ctx.root,
        ctx.ingest,
        ctx.interner,
        ctx.cli.page_request(),
        out,
    )
}
