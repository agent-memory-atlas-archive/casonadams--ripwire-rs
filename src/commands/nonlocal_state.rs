//! `--nonlocal-state`: reachable nonlocal mutable state read or written per function.
use super::{Plugin, Run};
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct NonlocalStateArgs {
    /// Reachable nonlocal mutable state read or written per function
    #[arg(long = "nonlocal-state")]
    pub nonlocal_state: bool,
}

pub fn render(ctx: &ExecutionContext<'_>, out: &mut XmlWriter<'_, impl Write>) -> io::Result<()> {
    let funcs = crate::metrics::nonlocal::find_nonlocal_state(ctx.ingest, ctx.graph, ctx.interner);
    crate::metrics::nonlocal::serialize_nonlocal_state_with_gauge_xml(
        ctx.root,
        &funcs,
        ctx.graph.gauges(),
        out,
    )
}

fn run(run: &Run<'_>, _args: &NonlocalStateArgs) -> io::Result<()> {
    run.report(render)
}

pub static NONLOCAL_STATE: Plugin<NonlocalStateArgs> = Plugin {
    name: "nonlocal-state",
    options: &[],
    selected: |args| args.nonlocal_state,
    run,
    check: None,
    tools: &[],
};
