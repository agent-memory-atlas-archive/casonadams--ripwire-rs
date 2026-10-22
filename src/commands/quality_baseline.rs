//! `--quality-baseline`: pins the quality floor stamped with git HEAD; MCP `quality_baseline`.
use super::common::gate::{validate_cli_gate_modifiers, with_workspace};
use super::common::mcp::mcp_tool;
use super::{Plugin, Run};
use crate::query::commands::{QueryContext, render_xml};
use crate::serialize::JsonValueExt;
use clap::Args;
use serde_json::Value;
use std::io;
use std::path::Path;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct QualityBaselineArgs {
    /// Pin quality baseline stamped with git HEAD
    #[arg(long)]
    pub quality_baseline: bool,
}

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct QualityBaselineOpts {
    /// Allow quality-baseline on a dirty working tree
    #[arg(long = "allow-dirty")]
    pub allow_dirty: bool,
}

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct QualityBaselineFlags {
    #[command(flatten)]
    pub primary: QualityBaselineArgs,
    #[command(flatten)]
    pub opts: QualityBaselineOpts,
}

/// Pins the quality baseline for the repository at `run.root`.
pub fn run_quality_baseline(run: &Run<'_>, allow_dirty: bool) -> io::Result<()> {
    with_workspace(run.cli, run.root, false, |state, out| {
        let sources = crate::query::FileSources {
            root: Path::new(&state.root),
        };
        crate::query::safety::execute_quality_baseline_with(
            &state.query_context(&sources),
            allow_dirty,
            out,
        )
    })
}

fn run(run: &Run<'_>, args: &QualityBaselineFlags) -> io::Result<()> {
    validate_cli_gate_modifiers(run.cli)?;
    run_quality_baseline(run, args.opts.allow_dirty)
}

fn call_quality_baseline(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    render_xml(|out| {
        crate::query::safety::execute_quality_baseline_with(
            state,
            args.bool_or("allow_dirty", false),
            out,
        )
    })
}

pub static QUALITY_BASELINE: Plugin<QualityBaselineFlags> = Plugin {
    name: "quality_baseline",
    options: &[],
    selected: |args| args.primary.quality_baseline || args.opts.allow_dirty,
    run,
    check: None,
    tools: &[mcp_tool!(
        "quality_baseline",
        "PIN the quality floor: writes .ripwire-rs_quality_baseline stamped with git HEAD sha",
        &[(
            "allow_dirty",
            "boolean",
            "Pin even when the dirty tree already holds gating findings against HEAD; the count is stamped as absorbed",
        )],
        &[],
        call_quality_baseline
    )],
};
