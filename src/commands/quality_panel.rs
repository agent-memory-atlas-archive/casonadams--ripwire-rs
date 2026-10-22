//! `--quality-panel [PRESET]`: the entire quality panel in one report.
use super::{Plugin, Run, SharedOption};
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct QualityPanelArgs {
    /// Entire quality panel in one report (strict|default|lenient)
    #[arg(long = "quality-panel", num_args = 0..=1, default_missing_value = "default", value_name = "PRESET")]
    pub quality_panel: Option<Option<String>>,
}

pub static QUALITY_PANEL: Plugin<QualityPanelArgs> = Plugin {
    name: "quality-panel",
    options: &[SharedOption::Paged],
    selected: |a| a.quality_panel.is_some(),
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, args: &QualityPanelArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    args: &QualityPanelArgs,
    out: &mut XmlWriter<'_, impl std::io::Write>,
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
