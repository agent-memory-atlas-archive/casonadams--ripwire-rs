//! `--affected SPECS`: test files affected by changes; MCP `affected`.
use super::common::gate::{validate_cli_gate_modifiers, with_workspace};
use super::common::mcp::mcp_tool;
use super::common::safety::safety_context;
use super::{Plugin, Run};
use crate::ToXmlString;
use crate::query::commands::{QueryContext, render_xml};
use crate::serialize::JsonValueExt;
use clap::Args;
use serde_json::Value;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct AffectedArgs {
    /// Show test files affected by changes
    #[arg(long)]
    pub affected: Option<String>,
}

/// Runs the affected-tests query for `specs`.
pub fn run_affected(run: &Run<'_>, specs: &str) -> io::Result<()> {
    with_workspace(run.cli, run.root, false, |state, out| {
        crate::query::safety::execute_affected_with(specs, &state.safety_context(), out)
    })
}

fn run(run: &Run<'_>, args: &AffectedArgs) -> io::Result<()> {
    validate_cli_gate_modifiers(run.cli)?;
    run_affected(run, args.affected.as_deref().unwrap_or_default())
}

fn call_affected(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let files = args.req_str("files")?;
    let context = safety_context(state);
    render_xml(|out| {
        crate::query::safety::affected_from_sources(files, &context, state.sources).write_xml(out)
    })
}

pub static AFFECTED: Plugin<AffectedArgs> = Plugin {
    name: "affected",
    options: &[],
    selected: |args| args.affected.is_some(),
    run,
    check: None,
    tools: &[mcp_tool!(
        "affected",
        "WHICH TESTS TO RUN for a change \u{2014} test files that transitively reach the changed files/symbols",
        &[(
            "files",
            "string",
            "Changed files or symbols, comma-separated"
        )],
        &["files"],
        call_affected
    )],
};
