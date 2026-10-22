//! `--uses SYM`: every statically resolvable use-site of a symbol.
use super::SharedOption;
use super::nav::nav_tool;
use super::plugin::report_command;
use crate::query::navigation::NavMode;

report_command! {
    name: "uses",
    args: UsesArgs,
    static: USES,
    options: [
        SharedOption::Paged,
        SharedOption::Budget("Output budget in tokens"),
    ],
    tools: [
        nav_tool!(
            "uses",
            "All statically resolvable use-sites of a symbol: call, read, write, import, extends",
            "Symbol name",
            NavMode::Uses
        ),
    ],
    run: nav(NavMode::Uses),
    /// Query all static use-sites of a symbol
    #[arg(long, value_name = "SYM")]
    uses: Option<String>,
}
