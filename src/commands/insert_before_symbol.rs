//! `--insert-before-symbol SYM:TEXT`: insert text immediately before a symbol's definition.
use super::common::edit::{EDIT_INS_PARAMS, call_edit_tool, run_edit};
use super::common::mcp::mcp_tool;
use super::{Plugin, Run};
use crate::query::edit::EditOp;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct InsertBeforeSymbolArgs {
    /// Insert text immediately before a symbol definition
    #[arg(long = "insert-before-symbol", value_name = "SYM:TEXT")]
    pub insert_before_symbol: Option<String>,
}

pub static INSERT_BEFORE_SYMBOL: Plugin<InsertBeforeSymbolArgs> = Plugin {
    name: "insert-before-symbol",
    options: &[],
    selected: |args| args.insert_before_symbol.is_some(),
    run,
    check: None,
    tools: &[mcp_tool!(
        "insert_before_symbol",
        "Insert text immediately BEFORE a symbol's definition (atomic write-rename)",
        EDIT_INS_PARAMS,
        &["symbol", "text"],
        |s, a| call_edit_tool(s, EditOp::InsertBefore, a, false)
    )],
};

fn run(run: &Run<'_>, args: &InsertBeforeSymbolArgs) -> io::Result<()> {
    run_edit(
        run,
        args.insert_before_symbol.as_deref().unwrap_or_default(),
        EditOp::InsertBefore,
    )
}
