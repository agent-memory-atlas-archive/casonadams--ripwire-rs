//! `--replace-symbol-body SYM:BODY`: replace a symbol's whole definition (atomic write-rename).
use super::common::edit::call_edit_tool;
use super::common::mcp::mcp_tool;
use super::{Plugin, Run};
use crate::query::edit::EditOp;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct ReplaceSymbolBodyArgs {
    /// Replace symbol definition body with text
    #[arg(long = "replace-symbol-body", value_name = "SYM:BODY")]
    pub replace_symbol_body: Option<String>,
}

pub static REPLACE_SYMBOL_BODY: Plugin<ReplaceSymbolBodyArgs> = Plugin {
    name: "replace-symbol-body",
    options: &[],
    selected: |args| args.replace_symbol_body.is_some(),
    run,
    check: None,
    tools: &[mcp_tool!(
        "replace_symbol_body",
        "Replace a symbol's ENTIRE definition with new_body (atomic write-rename)",
        &[
            ("symbol", "string", "Symbol name"),
            ("new_body", "string", "Replacement definition"),
            ("path", "string", "Optional file path hint"),
        ],
        &["symbol", "new_body"],
        |s, a| call_edit_tool(s, EditOp::ReplaceBody, a, true)
    )],
};

fn run(run: &Run<'_>, args: &ReplaceSymbolBodyArgs) -> io::Result<()> {
    super::common::edit::run_edit(
        run,
        args.replace_symbol_body.as_deref().unwrap_or_default(),
        EditOp::ReplaceBody,
    )
}
