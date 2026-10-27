//! `--callees SYM`: direct (1-hop) callees of a symbol.
use super::SharedOption;
use super::nav::nav_tool;
use super::plugin::report_command;
use crate::query::navigation::NavMode;

report_command! {
    name: "callees",
    args: CalleesArgs,
    static: CALLEES,
    options: [
        SharedOption::Paged,
        SharedOption::Budget("Output budget in tokens"),
    ],
    tools: [
        nav_tool!(
            "callees",
            "What this function or method calls: each call resolved to its definition, with calls that could not be bound listed as declined",
            NavMode::Callees
        ),
    ],
    run: nav(NavMode::Callees),
    /// Query 1-hop callees of a symbol
    #[arg(long, value_name = "SYM")]
    callees: Option<String>,
}
