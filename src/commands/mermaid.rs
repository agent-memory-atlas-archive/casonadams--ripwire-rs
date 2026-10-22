//! `--mermaid`: Mermaid flowchart of the module call graph, always plain markdown.
use super::{Plugin, Run};
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct MermaidArgs {
    /// Emit Mermaid flowchart diagram of module call graph
    #[arg(long = "mermaid")]
    pub mermaid: bool,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &MermaidArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    crate::graph::generate_mermaid_diagram(ctx.ingest, ctx.graph, ctx.interner, out)
}

fn run(run: &Run<'_>, args: &MermaidArgs) -> io::Result<()> {
    run.report_plain(|ctx, out| render(ctx, args, out))
}

pub static MERMAID: Plugin<MermaidArgs> = Plugin {
    name: "mermaid",
    options: &[],
    selected: |args| args.mermaid,
    run,
    check: None,
    tools: &[],
};
