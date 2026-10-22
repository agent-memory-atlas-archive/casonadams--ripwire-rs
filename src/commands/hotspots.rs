//! `--hotspots`: top maintenance hotspots (churn * ccx), paged by `--limit`/`--offset`.
use super::cochange::CochangeFlags;
use super::common::history::execute_with_git_state_fmt;
use super::context::OutputDialect;
use super::{Plugin, Run, SharedOption};
use crate::serialize::PageRequest;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct HotspotsArgs {
    /// Show top maintenance hotspots (churn * ccx)
    #[arg(long)]
    pub hotspots: bool,
}

/// Ranks maintenance hotspots over a 12-month git window rooted at `root`.
pub fn execute_hotspots(
    root: &str,
    since: Option<&str>,
    pretty: bool,
    json_out: bool,
    page: PageRequest,
) -> io::Result<()> {
    let cutoff = crate::git::history::resolve_cutoff_timestamp(since, 12);
    execute_with_git_state_fmt(root, cutoff, pretty, json_out, |commits, state, out| {
        crate::query::safety::execute_hotspots_with(
            since,
            commits,
            &state.safety_context(),
            page,
            out,
        )
    })
}

fn run(run: &Run<'_>, _args: &HotspotsArgs) -> io::Result<()> {
    let flags = run.cli.plugins.decode::<CochangeFlags>().unwrap_or_default();
    execute_hotspots(
        run.root,
        flags.cochange.since.as_deref(),
        run.cli.is_pretty_out(),
        run.cli.is_json_out(),
        run.page(),
    )
}

pub static HOTSPOTS: Plugin<HotspotsArgs> = Plugin {
    name: "hotspots",
    options: &[SharedOption::Paged],
    selected: |args| args.hotspots,
    run,
    check: None,
    tools: &[],
};
