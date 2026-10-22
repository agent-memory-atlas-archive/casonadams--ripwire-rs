//! `--naming-locals`: local-variable naming consistency check.
use super::{Plugin, Run, SharedOption};
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct NamingLocalsArgs {
    /// Check local-variable naming consistency
    #[arg(long = "naming-locals")]
    pub naming_locals: bool,
}

pub static NAMING_LOCALS: Plugin<NamingLocalsArgs> = Plugin {
    name: "naming-locals",
    options: &[SharedOption::Paged],
    selected: |a| a.naming_locals,
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, args: &NamingLocalsArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    _args: &NamingLocalsArgs,
    out: &mut XmlWriter<'_, impl std::io::Write>,
) -> io::Result<()> {
    crate::metrics::serialize_naming_consistency_xml(
        crate::metrics::NamingScope { root: ctx.root, include_locals: true },
        ctx.ingest,
        ctx.interner,
        ctx.cli.page_request(),
        out,
    )
}
