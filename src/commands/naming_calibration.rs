//! `--naming-calibration`: calibrates naming lens rules against git rename history.
use super::plugin::report_command;
use super::SharedOption;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "naming-calibration",
    args: NamingCalibrationArgs,
    static: NAMING_CALIBRATION,
    options: [SharedOption::Paged],
    run: report(render),
    /// Calibrate naming lens rules against git rename history
    #[arg(long = "naming-calibration")]
    naming_calibration: bool,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &NamingCalibrationArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    crate::metrics::serialize_naming_calibration_xml(
        ctx.root,
        ctx.ingest,
        ctx.interner,
        ctx.cli.page_request(),
        out,
    )
}
