//! `--seams`: cross-directory call edges lacking test coverage (paged by `--limit`/`--offset`).
use super::{Plugin, Run, SharedOption};
use crate::serialize::XmlReport;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct SeamsArgs {
    /// Cross-directory call edges lacking test coverage
    #[arg(long = "seams")]
    pub seams: bool,
}

fn run(run: &Run<'_>, _args: &SeamsArgs) -> io::Result<()> {
    run.report(|ctx, out| {
        let rep = crate::gate::seams::find_integration_seams(
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            run.page(),
        );
        rep.write_xml_report(ctx.root, out)
    })
}

pub static SEAMS: Plugin<SeamsArgs> = Plugin {
    name: "seams",
    options: &[SharedOption::Paged],
    selected: |args| args.seams,
    run,
    check: None,
    tools: &[],
};
