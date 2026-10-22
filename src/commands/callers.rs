//! `--callers SYM`: direct (1-hop) callers of a symbol.
use super::nav::{nav_tool, run_nav};
use super::{Plugin, Run, SharedOption};
use crate::query::navigation::NavMode;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct CallersArgs {
    /// Query 1-hop callers of a symbol
    #[arg(long, value_name = "SYM")]
    pub callers: Option<String>,
}

pub static CALLERS: Plugin<CallersArgs> = Plugin {
    name: "callers",
    options: &[SharedOption::Paged],
    selected: |args| args.callers.is_some(),
    run,
    check: None,
    tools: &[
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
};

fn run(run: &Run<'_>, args: &CallersArgs) -> io::Result<()> {
    run_nav(run, args.callers.as_deref().unwrap_or_default(), NavMode::Callers)
}
