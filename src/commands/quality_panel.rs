//! `--quality-panel [PRESET]`: the entire quality panel in one report.
use super::SharedOption;
use super::plugin::report_command;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "quality-panel",
    args: QualityPanelArgs,
    static: QUALITY_PANEL,
    options: [SharedOption::Paged],
    run: report(render),
    /// Entire quality panel in one report (strict|default|lenient)
    #[arg(long = "quality-panel", num_args = 0..=1, default_missing_value = "default", value_name = "PRESET")]
    quality_panel: Option<Option<String>>,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    args: &QualityPanelArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let preset = args.quality_panel.as_ref().and_then(|p| p.as_deref());
    let rep = crate::metrics::compute_quality_panel_report(
        ctx.ingest,
        ctx.graph,
        ctx.interner,
        preset,
        ctx.cli.page_request(),
    );
    ctx.write_report(&rep, out)?;
    Ok(())
}
