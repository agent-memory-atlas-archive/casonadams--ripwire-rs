//! `--naming-calibration`: calibrates naming lens rules against git rename history.
use super::{Plugin, Run, SharedOption};
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct NamingCalibrationArgs {
    /// Calibrate naming lens rules against git rename history
    #[arg(long = "naming-calibration")]
    pub naming_calibration: bool,
}

pub static NAMING_CALIBRATION: Plugin<NamingCalibrationArgs> = Plugin {
    name: "naming-calibration",
    options: &[SharedOption::Paged],
    selected: |a| a.naming_calibration,
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, args: &NamingCalibrationArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    _args: &NamingCalibrationArgs,
    out: &mut XmlWriter<'_, impl std::io::Write>,
) -> io::Result<()> {
    crate::metrics::serialize_naming_calibration_xml(
        ctx.root,
        ctx.ingest,
        ctx.interner,
        ctx.cli.page_request(),
        out,
    )
}
