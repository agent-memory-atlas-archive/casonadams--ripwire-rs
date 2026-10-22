//! `--ensemble`: four orthogonal evidence families joined and ranked.
use super::plugin::report_command;
use super::SharedOption;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "ensemble",
    args: EnsembleArgs,
    static: ENSEMBLE,
    options: [SharedOption::Paged],
    run: report(render),
    /// Four orthogonal evidence families joined, ranked by distinct families fired
    #[arg(long = "ensemble")]
    ensemble: bool,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &EnsembleArgs,
    out: &mut XmlWriter<'_, W>,
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
