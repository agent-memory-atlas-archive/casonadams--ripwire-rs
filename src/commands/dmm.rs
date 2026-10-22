//! `--dmm [REV|A..B]`: Delta Maintainability Model scalar in [0,1].
use super::common::safety_queries::execute_dmm_query;
use super::plugin::report_command;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "dmm",
    args: DmmArgs,
    static: DMM,
    run: report(render),
    /// Delta Maintainability Model scalar in [0,1]
    #[arg(long = "dmm", num_args = 0..=1, default_missing_value = "", value_name = "REV|A..B")]
    dmm: Option<Option<String>>,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    args: &DmmArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let rev = args.dmm.clone().flatten();
    execute_dmm_query(
        rev.as_deref(),
        ctx.root,
        ctx.ingest,
        ctx.graph,
        ctx.interner,
        out,
    )
    .map(|_| ())
}
