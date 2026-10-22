//! `--edit-check SYM`: verifies 1-hop caller compatibility against git HEAD; MCP `edit_check`.
use super::common::gate::{validate_cli_gate_modifiers, with_workspace};
use super::common::mcp::mcp_tool;
use super::common::safety::safety_context;
use super::{Plugin, Run, SharedOption};
use crate::query::commands::{QueryContext, render_xml};
use crate::serialize::JsonValueExt;
use clap::Args;
use serde_json::Value;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct EditCheckArgs {
    /// Verify 1-hop caller compatibility against git HEAD
    #[arg(long)]
    pub edit_check: Option<String>,
}

/// Runs the edit-check gate for `symbol` over the paged window of `run`.
pub fn run_edit_check(run: &Run<'_>, symbol: &str) -> io::Result<()> {
    let page = run.page();
    let window = page.limit.map(|limit| (page.offset.unwrap_or(0), limit));
    with_workspace(run.cli, run.root, false, |state, out| {
        crate::query::safety::execute_edit_check_with(
            symbol,
            window,
            &state.safety_context(),
            out,
        )
    })
}

fn run(run: &Run<'_>, args: &EditCheckArgs) -> io::Result<()> {
    validate_cli_gate_modifiers(run.cli)?;
    run_edit_check(run, args.edit_check.as_deref().unwrap_or_default())
}

fn call_edit_check(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let symbol = args.req_str("symbol")?;
    let context = safety_context(state);
    let page = args
        .opt_usize("limit")
        .map(|limit| (args.opt_usize("offset").unwrap_or(0), limit));
    render_xml(|out| crate::query::safety::execute_edit_check_with(symbol, page, &context, out))
}

pub static EDIT_CHECK: Plugin<EditCheckArgs> = Plugin {
    name: "edit_check",
    options: &[SharedOption::Paged],
    selected: |args| args.edit_check.is_some(),
    run,
    check: None,
    tools: &[mcp_tool!(
        "edit_check",
        "After an edit: did SYM's contract change, and which callers no longer fit?",
        &[
            ("symbol", "string", "Symbol name to verify"),
        ],
        &["symbol"],
        call_edit_check
    )],
};
