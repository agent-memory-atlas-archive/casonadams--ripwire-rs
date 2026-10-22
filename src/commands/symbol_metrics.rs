//! `--metrics`: annotate every symbol with fan-in, fan-out, complexity, and change-amplification.
use super::{Plugin, Run};
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct SymbolMetricsArgs {
    /// Annotate every symbol with fan-in, fan-out, complexity, and change-amplification
    #[arg(long = "metrics")]
    pub metrics: bool,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &SymbolMetricsArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let opts = crate::serialize::SerializeOptions {
        root: ctx.root.to_string(),
        top_k: ctx.cli.top_k,
        pack_budget_bytes: if ctx.cli.top_k.is_some() {
            None
        } else {
            Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES)
        },
        metrics: true,
        ..Default::default()
    };
    crate::serialize::serialize_xml(ctx.ingest, ctx.graph, ctx.pr, ctx.interner, out, &opts)
}

fn run(run: &Run<'_>, args: &SymbolMetricsArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub static SYMBOL_METRICS: Plugin<SymbolMetricsArgs> = Plugin {
    name: "metrics",
    options: &[],
    selected: |args| args.metrics,
    run,
    check: None,
    tools: &[],
};
