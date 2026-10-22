//! `--callees SYM`: direct (1-hop) callees of a symbol.
use super::nav::{nav_tool, run_nav};
use super::{Plugin, Run, SharedOption};
use crate::query::navigation::NavMode;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct CalleesArgs {
    /// Query 1-hop callees of a symbol
    #[arg(long, value_name = "SYM")]
    pub callees: Option<String>,
}

pub static CALLEES: Plugin<CalleesArgs> = Plugin {
    name: "callees",
    options: &[SharedOption::Paged],
    selected: |args| args.callees.is_some(),
    run,
    check: None,
    tools: &[nav_tool!(
        "callees",
        "Direct (1-hop) callees of a symbol",
        "Symbol name to look up",
        NavMode::Callees
    )],
};

fn run(run: &Run<'_>, args: &CalleesArgs) -> io::Result<()> {
    run_nav(run, args.callees.as_deref().unwrap_or_default(), NavMode::Callees)
}
