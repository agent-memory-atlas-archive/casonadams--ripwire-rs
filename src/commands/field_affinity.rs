//! `--field-affinity [STRUCT]`: cache-conscious field affinity analysis.
use super::{Plugin, Run};
use crate::commands::context::ExecutionContext;
use crate::commands::context::Paginated;
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct FieldAffinityArgs {
    /// Cache-conscious field affinity analysis
    #[arg(long = "field-affinity", num_args = 0..=1, default_missing_value = "", value_name = "STRUCT")]
    pub field_affinity: Option<Option<String>>,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    args: &FieldAffinityArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    if let Some(sym_opt) = &args.field_affinity {
        let rep = crate::metrics::layout::compute_field_affinity_report(
            sym_opt.as_deref(),
            ctx.ingest,
            ctx.interner,
            ctx.root,
            ctx.cli.page_request(),
        );
        ctx.write_report(&rep, out)?;
    }
    Ok(())
}

fn run(run: &Run<'_>, args: &FieldAffinityArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub static FIELD_AFFINITY: Plugin<FieldAffinityArgs> = Plugin {
    name: "field-affinity",
    options: &[],
    selected: |args| args.field_affinity.is_some(),
    run,
    check: None,
    tools: &[],
};
