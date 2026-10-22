//! `--outline SYMS`: definition skeletons of symbols; reads `--compress` through `--expand`.
use super::{Plugin, Run};
use crate::commands::common::ladder::execute_expand_or_outline_compressed;
use crate::commands::context::ExecutionContext;
use crate::commands::expand;
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct OutlineArgs {
    #[arg(long, value_name = "SYMS")]
    pub outline: Option<String>,
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    args: &OutlineArgs,
    out: &mut XmlWriter<'_, impl Write>,
) -> io::Result<()> {
    execute_expand_or_outline_compressed(
        ctx.root,
        args.outline.as_deref().unwrap_or_default(),
        true,
        expand::compress(ctx.cli),
        ctx.ingest,
        ctx.interner,
        out,
    )
}

fn run(run: &Run<'_>, args: &OutlineArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub static OUTLINE: Plugin<OutlineArgs> = Plugin {
    name: "outline",
    options: &[],
    selected: |args| args.outline.is_some(),
    run,
    check: None,
    tools: &[],
};
