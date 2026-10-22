//! `--expand SYMS`: full symbol bodies; `--compress` (also read by `--outline`) selects the compact dialect.
use super::{Plugin, Run};
use crate::cli::Cli;
use crate::commands::common::ladder::execute_expand_or_outline_compressed;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct ExpandArgs {
    /// Compress output using compact token dialect
    #[arg(long = "compress", overrides_with = "compress")]
    pub compress: bool,
    /// Expand full symbol bodies and callee signatures (SYM,SYM...)
    #[arg(long, value_name = "SYMS")]
    pub expand: Option<String>,
}

/// `--compress`, shared by `--expand` and `--outline`.
#[must_use]
pub fn compress(cli: &Cli) -> bool {
    cli.plugins.decode::<ExpandArgs>().is_some_and(|args| args.compress)
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    args: &ExpandArgs,
    out: &mut XmlWriter<'_, impl Write>,
) -> io::Result<()> {
    execute_expand_or_outline_compressed(
        ctx.root,
        args.expand.as_deref().unwrap_or_default(),
        false,
        args.compress,
        ctx.ingest,
        ctx.interner,
        out,
    )
}

fn run(run: &Run<'_>, args: &ExpandArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub static EXPAND: Plugin<ExpandArgs> = Plugin {
    name: "expand",
    options: &[],
    selected: |args| args.expand.is_some(),
    run,
    check: None,
    tools: &[],
};
