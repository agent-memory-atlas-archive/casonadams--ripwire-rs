//! `--pin-census`: pins the quality census snapshot as the quality baseline sidecar.
use super::common::pin::pin_quality_baseline;
use super::common::safety::context_of;
use super::{Plugin, Run};
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct PinCensusArgs {
    /// Pin quality census snapshot
    #[arg(long = "pin-census")]
    pub pin_census: bool,
}

fn run(run: &Run<'_>, _args: &PinCensusArgs) -> io::Result<()> {
    run.report(|ctx, out| pin_quality_baseline(&context_of(ctx), out))
}

pub static PIN_CENSUS: Plugin<PinCensusArgs> = Plugin {
    name: "pin-census",
    options: &[],
    selected: |args| args.pin_census,
    run,
    check: None,
    tools: &[],
};
