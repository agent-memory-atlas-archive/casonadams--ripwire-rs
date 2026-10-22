//! `--uses SYM`: every statically resolvable use-site of a symbol.
use super::nav::{nav_tool, run_nav};
use super::{Plugin, Run, SharedOption};
use crate::query::navigation::NavMode;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct UsesArgs {
    /// Query all static use-sites of a symbol
    #[arg(long, value_name = "SYM")]
    pub uses: Option<String>,
}

pub static USES: Plugin<UsesArgs> = Plugin {
    name: "uses",
    options: &[SharedOption::Paged],
    selected: |args| args.uses.is_some(),
    run,
    check: None,
    tools: &[nav_tool!(
        "uses",
        "All statically resolvable use-sites of a symbol: call, read, write, import, extends",
        "Symbol name",
        NavMode::Uses
    )],
};

fn run(run: &Run<'_>, args: &UsesArgs) -> io::Result<()> {
    run_nav(run, args.uses.as_deref().unwrap_or_default(), NavMode::Uses)
}
