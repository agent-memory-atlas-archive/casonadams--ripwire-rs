//! `--outline SYMS`: definition skeletons of symbols; reads `--compress` through `--expand`.
use super::plugin::report_command;
use crate::commands::common::ladder::execute_expand_or_outline_compressed;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "outline",
    args: OutlineArgs,
    static: OUTLINE,
    run: report(render),
    #[arg(long, value_name = "SYMS")]
    outline: Option<String>,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    args: &OutlineArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    execute_expand_or_outline_compressed(
        ctx.root,
        args.outline.as_deref().unwrap_or_default(),
        true,
        ctx.cli.modifiers.compress,
        ctx.ingest,
        ctx.interner,
        out,
    )
}
