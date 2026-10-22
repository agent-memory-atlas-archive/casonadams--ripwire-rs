//! `--insert-after-symbol SYM:TEXT`: insert text immediately after a symbol's definition.
use super::common::edit::{EDIT_INS_PARAMS, call_edit_tool, run_edit};
use super::common::mcp::mcp_tool;
use super::{Plugin, Run};
use crate::query::edit::EditOp;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct InsertAfterSymbolArgs {
    /// Insert text immediately after a symbol definition
    #[arg(long = "insert-after-symbol", value_name = "SYM:TEXT")]
    pub insert_after_symbol: Option<String>,
}

pub static INSERT_AFTER_SYMBOL: Plugin<InsertAfterSymbolArgs> = Plugin {
    name: "insert-after-symbol",
    options: &[],
    selected: |args| args.insert_after_symbol.is_some(),
    run,
    check: None,
    tools: &[mcp_tool!(
        "insert_after_symbol",
        "Insert text immediately AFTER a symbol's definition (atomic write-rename)",
        EDIT_INS_PARAMS,
        &["symbol", "text"],
        |s, a| call_edit_tool(s, EditOp::InsertAfter, a, false)
    )],
};

fn run(run: &Run<'_>, args: &InsertAfterSymbolArgs) -> io::Result<()> {
    run_edit(
        run,
        args.insert_after_symbol.as_deref().unwrap_or_default(),
        EditOp::InsertAfter,
    )
}
