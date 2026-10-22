//! `--naming-consistency`: naming convention consistency check.
use super::{Plugin, Run, SharedOption};
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct NamingConsistencyArgs {
    /// Naming convention consistency check
    #[arg(long = "naming-consistency")]
    pub naming_consistency: bool,
}

pub static NAMING_CONSISTENCY: Plugin<NamingConsistencyArgs> = Plugin {
    name: "naming-consistency",
    options: &[SharedOption::Paged],
    selected: |a| a.naming_consistency,
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, args: &NamingConsistencyArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    _args: &NamingConsistencyArgs,
    out: &mut XmlWriter<'_, impl std::io::Write>,
) -> io::Result<()> {
    crate::metrics::serialize_naming_consistency_xml(
        crate::metrics::NamingScope { root: ctx.root, include_locals: false },
        ctx.ingest,
        ctx.interner,
        ctx.cli.page_request(),
        out,
    )
}
