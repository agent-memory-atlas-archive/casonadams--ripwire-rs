//! `--metrics`: annotate every symbol with fan-in, fan-out, complexity, and change-amplification.
use super::plugin::report_command;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "metrics",
    args: SymbolMetricsArgs,
    static: SYMBOL_METRICS,
    run: report(render),
    /// Annotate every symbol with fan-in, fan-out, complexity, and change-amplification
    #[arg(long = "metrics")]
    metrics: bool,
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
