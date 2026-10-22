//! `--merge-scout REF1,REF2,...`: cross-branch symbol conflict predictions across concurrent branch refs.
use super::common::safety::report_exit;
use super::common::safety_queries::execute_merge_scout_dispatch;
use super::{Plugin, Run};
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct MergeScoutArgs {
    /// Cross-branch symbol conflict predictions across concurrent branch refs
    #[arg(long = "merge-scout", value_name = "REF1,REF2,...")]
    pub merge_scout: Option<String>,
}

fn run(run: &Run<'_>, args: &MergeScoutArgs) -> io::Result<()> {
    let branches = args.merge_scout.as_deref().unwrap_or_default();
    report_exit(run, |ctx, out| {
        execute_merge_scout_dispatch(ctx.root, branches, ctx.ingest, ctx.interner, out)
    })
}

pub static MERGE_SCOUT: Plugin<MergeScoutArgs> = Plugin {
    name: "merge-scout",
    options: &[],
    selected: |args| args.merge_scout.is_some(),
    run,
    check: None,
    tools: &[],
};
