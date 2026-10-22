//! `--impact SYM`: transitive blast radius of a symbol; `--impact-depth` bounds every navigation query.
use super::nav::{nav_tool, run_nav};
use super::{Plugin, Run, SharedOption};
use crate::query::navigation::NavMode;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct ImpactArgs {
    /// Query transitive impact blast radius of a symbol
    #[arg(long, value_name = "SYM")]
    pub impact: Option<String>,
}


pub static IMPACT: Plugin<ImpactArgs> = Plugin {
    name: "impact",
    options: &[
        SharedOption::Paged,
        SharedOption::Budget("Output budget in tokens"),
    ],
    selected: |args| args.impact.is_some(),
    run,
    check: None,
    tools: &[nav_tool!(
        "impact",
        "Transitive blast radius of a symbol via calls, ranked by PageRank",
        "Symbol name to analyze blast radius for",
        NavMode::Impact
    )],
};

fn run(run: &Run<'_>, args: &ImpactArgs) -> io::Result<()> {
    run_nav(run, args.impact.as_deref().unwrap_or_default(), NavMode::Impact)
}
