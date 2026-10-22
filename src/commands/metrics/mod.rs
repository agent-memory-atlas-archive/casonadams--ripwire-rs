//! Metrics group: `--cochange`, `--hotspots`, `--since` and the `metrics` subcommand. The git history
//! analyses live with the safety group.

use super::{Command, decode_subcommand};
use crate::cli::{Cli, OutputDialect, Paginated};
use crate::commands::safety::{GitMetricsFlags, dispatch_git_metrics_with_flags};
use clap::{Args, Command as ClapCommand};
use std::io;

/// Top-level git history metric flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct MetricsFlags {
    /// Fowler's Shotgun Surgery co-change mining
    #[arg(long)]
    pub cochange: Option<Option<String>>,

    /// Limit git history traversal to commits after this date or revision
    #[arg(long)]
    pub since: Option<String>,

    /// Show top maintenance hotspots (churn * ccx)
    #[arg(long)]
    pub hotspots: bool,
}

/// `metrics` subcommand: git history metrics scoped to a repository path.
#[derive(Args, Debug, Default, PartialEq)]
pub struct MetricsArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

    /// Show top maintenance hotspots (churn * ccx)
    #[arg(long)]
    pub hotspots: bool,

    /// Show file co-change coupling (Fowler's Shotgun Surgery)
    #[arg(long)]
    pub cochange: Option<Option<String>>,

    /// Limit git history traversal to commits after this date or revision
    #[arg(long)]
    pub since: Option<String>,
}

fn execute_metrics_subcommand(args: &MetricsArgs, default_root: &str) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(default_root);
    dispatch_git_metrics_with_flags(
        root,
        GitMetricsFlags {
            cochange: args.cochange.as_ref().map(|value| value.as_deref()),
            hotspots: args.hotspots,
            since: args.since.as_deref(),
            ..Default::default()
        },
    )
    .unwrap_or(Ok(()))
}

pub struct Metrics;

pub static METRICS: Metrics = Metrics;

impl Command for Metrics {
    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        MetricsFlags::augment_args(cmd)
    }

    fn subcommands(&self) -> Vec<ClapCommand> {
        vec![
            MetricsArgs::augment_args(ClapCommand::new("metrics"))
                .about("Code complexity, churn, and architectural health metrics"),
        ]
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let flags = cli.plugins.decode::<MetricsFlags>()?;
        let safety = crate::commands::safety::args(cli);
        dispatch_git_metrics_with_flags(
            root,
            GitMetricsFlags {
                cochange: flags.cochange.as_ref().map(|o| o.as_deref()),
                cochange_groups: safety.cochange_groups,
                cochange_recur: safety.cochange_recur,
                hotspots: flags.hotspots,
                since: flags.since.as_deref(),
                pretty: cli.is_pretty_out(),
                json_out: cli.is_json_out(),
                limit: cli.limit_opt(),
                offset: cli.offset_opt(),
            },
        )
    }

    fn run_subcommand(
        &self,
        _cli: &Cli,
        name: &str,
        matches: &clap::ArgMatches,
        root: &str,
    ) -> Option<io::Result<()>> {
        if name != "metrics" {
            return None;
        }
        Some(
            decode_subcommand::<MetricsArgs>(matches)
                .and_then(|args| execute_metrics_subcommand(&args, root)),
        )
    }
}
