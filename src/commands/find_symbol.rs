//! `--find-symbol SYM`: a symbol's 1-hop neighborhood, callers and callees.
use super::SharedOption;
use super::nav::nav_tool;
use super::plugin::report_command;
use crate::query::navigation::NavMode;

report_command! {
    name: "find-symbol",
    args: FindSymbolArgs,
    static: FIND_SYMBOL,
    options: [SharedOption::Paged],
    tools: [
        nav_tool!(
            "find_symbol",
            "A symbol's 1-hop neighborhood: the symbol plus direct callers and callees",
            "Symbol name to look up",
            NavMode::FindSymbol
        ),
    ],
    run: nav(NavMode::FindSymbol),
    /// Query 1-hop callers and callees of a symbol
    #[arg(long = "find-symbol", value_name = "SYM")]
    find_symbol: Option<String>,
}
