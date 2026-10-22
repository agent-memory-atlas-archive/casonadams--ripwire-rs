//! `--callees SYM`: direct (1-hop) callees of a symbol.
use super::SharedOption;
use super::nav::nav_tool;
use super::plugin::report_command;
use crate::query::navigation::NavMode;

report_command! {
    name: "callees",
    args: CalleesArgs,
    static: CALLEES,
    options: [SharedOption::Paged],
    tools: [
        nav_tool!(
            "callees",
            "Direct (1-hop) callees of a symbol",
            "Symbol name to look up",
            NavMode::Callees
        ),
    ],
    run: nav(NavMode::Callees),
    /// Query 1-hop callees of a symbol
    #[arg(long, value_name = "SYM")]
    callees: Option<String>,
}
