//! `--find-symbol SYM`: a symbol's 1-hop neighborhood, callers and callees.
use super::nav::{nav_tool, run_nav};
use super::{Plugin, Run, SharedOption};
use crate::query::navigation::NavMode;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct FindSymbolArgs {
    /// Query 1-hop callers and callees of a symbol
    #[arg(long = "find-symbol", value_name = "SYM")]
    pub find_symbol: Option<String>,
}

pub static FIND_SYMBOL: Plugin<FindSymbolArgs> = Plugin {
    name: "find-symbol",
    options: &[SharedOption::Paged],
    selected: |args| args.find_symbol.is_some(),
    run,
    check: None,
    tools: &[nav_tool!(
        "find_symbol",
        "A symbol's 1-hop neighborhood: the symbol plus direct callers and callees",
        "Symbol name to look up",
        NavMode::FindSymbol
    )],
};

fn run(run: &Run<'_>, args: &FindSymbolArgs) -> io::Result<()> {
    run_nav(run, args.find_symbol.as_deref().unwrap_or_default(), NavMode::FindSymbol)
}
