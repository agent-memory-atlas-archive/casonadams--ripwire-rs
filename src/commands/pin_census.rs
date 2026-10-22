//! `--pin-census`: pins the quality census snapshot as the quality baseline sidecar.
use super::common::pin::pin_quality_baseline;
use super::common::safety::context_of;
use super::plugin::report_command;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "pin-census",
    args: PinCensusArgs,
    static: PIN_CENSUS,
    run: report(render),
    /// Pin quality census snapshot
    #[arg(long = "pin-census")]
    pin_census: bool,
}

fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &PinCensusArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    pin_quality_baseline(&context_of(ctx), out)
}
