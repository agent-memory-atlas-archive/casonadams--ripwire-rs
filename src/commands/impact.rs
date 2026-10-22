//! `--impact SYM`: transitive blast radius of a symbol; `--impact-depth` bounds every navigation query.
use super::nav::{nav_tool, run_nav};
use super::{Plugin, Run, SharedOption};
use crate::cli::Cli;
use crate::query::navigation::NavMode;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct ImpactArgs {
    /// Query transitive impact blast radius of a symbol
    #[arg(long, value_name = "SYM")]
    pub impact: Option<String>,
    /// Limit transitive impact traversal to N call hops
    #[arg(long = "impact-depth", global = true, value_name = "N")]
    pub impact_depth: Option<u32>,
}

/// `--impact-depth`, read by every navigation query and the `navigate` subcommand.
#[must_use]
pub fn depth(cli: &Cli) -> Option<u32> {
    cli.plugins.decode::<ImpactArgs>()?.impact_depth
}

pub static IMPACT: Plugin<ImpactArgs> = Plugin {
    name: "impact",
    options: &[SharedOption::Paged],
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
