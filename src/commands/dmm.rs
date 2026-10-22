//! `--dmm [REV|A..B]`: Delta Maintainability Model scalar in [0,1].
use super::common::safety_queries::execute_dmm_query;
use super::{Plugin, Run};
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct DmmArgs {
    /// Delta Maintainability Model scalar in [0,1]
    #[arg(long = "dmm", num_args = 0..=1, default_missing_value = "", value_name = "REV|A..B")]
    pub dmm: Option<Option<String>>,
}

fn run(run: &Run<'_>, args: &DmmArgs) -> io::Result<()> {
    let rev = args.dmm.clone().flatten();
    run.report(|ctx, out| {
        execute_dmm_query(
            rev.as_deref(),
            ctx.root,
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            out,
        )
        .map(|_| ())
    })
}

pub static DMM: Plugin<DmmArgs> = Plugin {
    name: "dmm",
    options: &[],
    selected: |args| args.dmm.is_some(),
    run,
    check: None,
    tools: &[],
};
