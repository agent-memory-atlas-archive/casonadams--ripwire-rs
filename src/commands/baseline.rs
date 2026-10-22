//! `--baseline` / `--baseline-update`: pins or updates the architectural baseline sidecar.
use super::common::pin::pin_quality_baseline;
use super::common::safety::context_of;
use super::{Plugin, Run};
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct BaselineArgs {
    /// Pin architectural baseline sidecar
    #[arg(long = "baseline")]
    pub baseline: bool,
    /// Update architectural baseline sidecar with new violations
    #[arg(long = "baseline-update")]
    pub baseline_update: bool,
}

fn run(run: &Run<'_>, _args: &BaselineArgs) -> io::Result<()> {
    run.report(|ctx, out| pin_quality_baseline(&context_of(ctx), out))
}

pub static BASELINE: Plugin<BaselineArgs> = Plugin {
    name: "baseline",
    options: &[],
    selected: |args| args.baseline || args.baseline_update,
    run,
    check: None,
    tools: &[],
};
