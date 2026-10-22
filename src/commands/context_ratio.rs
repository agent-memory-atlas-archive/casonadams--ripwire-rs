//! `--context-ratio`: share of each unit's context lying outside its own file.
use super::plugin::report_command;
use super::SharedOption;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "context-ratio",
    args: ContextRatioArgs,
    static: CONTEXT_RATIO,
    options: [SharedOption::Paged],
    run: report(render),
    /// Local-reasoning lens measuring share of context outside a unit's file
    #[arg(long = "context-ratio")]
    context_ratio: bool,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &ContextRatioArgs,
    out: &mut XmlWriter<'_, W>,
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
