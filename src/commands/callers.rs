//! `--callers SYM`: direct (1-hop) callers of a symbol.
use super::SharedOption;
use super::nav::nav_tool;
use super::plugin::report_command;
use crate::query::navigation::NavMode;

report_command! {
    name: "callers",
    args: CallersArgs,
    static: CALLERS,
    options: [SharedOption::Paged],
    tools: [
        nav_tool!(
            "callers",
            "Direct (1-hop) callers of a symbol (alias for find_referencing_symbols)",
            "Symbol name to look up",
            NavMode::Callers
        ),
        nav_tool!(
            "find_referencing_symbols",
            "Direct (1-hop) callers of a symbol",
            "Symbol name to look up",
            NavMode::Callers
        ),
    ],
    run: nav(NavMode::Callers),
    /// Query 1-hop callers of a symbol
    #[arg(long, value_name = "SYM")]
    callers: Option<String>,
}
