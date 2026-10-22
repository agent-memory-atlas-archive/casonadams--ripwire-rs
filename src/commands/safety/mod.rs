//! Pre-PR safety group: `--test-gate`, `--situ`, `--safe-delete`, `--exercises`, `--seams`, `--pr-context`,
//! `--merge-scout`, `--dmm`, `--plan-lanes`, `--scan-skills`, `--edit-plan`, `--edit-payload`, `--partition`,
//! `--pin-census`, `--baseline`, `--baseline-update`, plus the MCP tools of its query and gate helpers.

mod execute;
mod tools;

pub use execute::*;

use super::Command;
use crate::cli::Cli;
use crate::commands::context::{CliVerb, ExecutionContext, VerbGroup, run_cli_verb};
use crate::query::commands::CommandDescriptor;
use clap::{Args, Command as ClapCommand};
use std::io::{self, Write};

/// Top-level pre-PR safety and verification flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct SafetyArgs {
    /// Before a PR: name tests to run and untested blast radius (exit 4 if non-empty)
    #[arg(long = "test-gate", value_name = "FILES")]
    pub test_gate: Option<Option<String>>,
    /// Situational awareness for a change: blast radius + tests + co-change (default: git diff)
    #[arg(long, value_name = "FILES")]
    pub situ: Option<Option<String>>,
    /// Safe delete assessment joining callers, blast radius, tests, and history
    #[arg(long = "safe-delete", value_name = "SYM")]
    pub safe_delete: Option<String>,
    /// Show what a test file actually covers: non-test symbols it reaches
    #[arg(long, value_name = "TESTFILE")]
    pub exercises: Option<String>,
    /// Re-rank map toward git-changed files so recent work floats up
    #[arg(long = "map-diff")]
    pub map_diff: bool,
    /// Cross-directory call edges lacking test coverage
    #[arg(long = "seams")]
    pub seams: bool,
    /// Review bundle per changed file vs base: symbols, callers, blast radius, tests
    #[arg(long = "pr-context", value_name = "REF")]
    pub pr_context: Option<Option<String>>,
    /// Cross-branch symbol conflict predictions across concurrent branch refs
    #[arg(long = "merge-scout", value_name = "REF1,REF2,...")]
    pub merge_scout: Option<String>,
    /// Acknowledge findings in quality-delta
    #[arg(long = "quality-ack", value_name = "REASON")]
    pub quality_ack: Option<Option<String>>,
    /// Narrow quality-ack to specific kinds or facets
    #[arg(long = "ack-only", value_name = "SUBSTR")]
    pub ack_only: Option<String>,
    /// Allow quality-baseline on a dirty working tree
    #[arg(long = "allow-dirty")]
    pub allow_dirty: bool,
    /// Show stale quality-delta acknowledgments even on clean runs
    #[arg(long = "show-stale")]
    pub show_stale: bool,
    /// Partition quality-delta by ownership scope
    #[arg(long = "scope", value_name = "SCOPE")]
    pub scope: Option<String>,
    /// Pin architectural baseline sidecar
    #[arg(long = "baseline")]
    pub baseline: bool,
    /// Update architectural baseline sidecar with new violations
    #[arg(long = "baseline-update")]
    pub baseline_update: bool,
    /// Cluster co-changing files into multi-file change groups
    #[arg(long = "cochange-groups")]
    pub cochange_groups: bool,
    /// Minimum sub-window recurrence count for co-change pairs
    #[arg(long = "cochange-recur", value_name = "K")]
    pub cochange_recur: Option<u32>,
    /// Target file for symbol edit disambiguation
    #[arg(long = "edit-target-file", value_name = "FILE")]
    pub edit_target_file: Option<String>,
    /// Skip post-edit contract and syntax verification
    #[arg(long = "no-post-check")]
    pub no_post_check: bool,
    /// Allow atomic symbol edits when connected over remote transport
    #[arg(long = "allow-remote-edits")]
    pub allow_remote_edits: bool,
    /// Pin quality census snapshot
    #[arg(long = "pin-census")]
    pub pin_census: bool,
    /// Partition codebase into cohesive modules
    #[arg(long = "partition", value_name = "K")]
    pub partition: Option<Option<usize>>,
    /// Delta Maintainability Model scalar in [0,1]
    #[arg(long = "dmm", num_args = 0..=1, default_missing_value = "", value_name = "REV|A..B")]
    pub dmm: Option<Option<String>>,
    /// Plan parallel worktree lanes before code (split across N worktrees)
    #[arg(long = "plan-lanes", num_args = 0..=1, default_missing_value = "2", value_name = "N")]
    pub plan_lanes: Option<Option<usize>>,
    /// Goal or task description for --plan-lanes
    #[arg(long = "task", value_name = "GOAL")]
    pub task: Option<String>,
    /// Explicit per-lane brief file for --plan-lanes (one lane task per line)
    #[arg(long = "brief", num_args = 0..=1, default_missing_value = "", value_name = "FILE")]
    pub brief: Option<String>,
    /// Security audit scanner for agent skill files
    #[arg(
        long = "scan-skills",
        visible_alias = "scan-skill",
        num_args = 0..=1,
        default_missing_value = "",
        value_name = "DIR"
    )]
    pub scan_skills: Option<Option<String>>,
    /// Apply atomic code mutations
    #[arg(long = "apply")]
    pub apply: bool,
    /// Validate edits without mutating files
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    /// Force edits bypassing safety checks
    #[arg(long = "force")]
    pub force: bool,
    /// Execute versioned edit plan
    #[arg(long = "edit-plan", value_name = "PLAN_JSON")]
    pub edit_plan: Option<String>,
    /// Execute JSON edit payload
    #[arg(long = "edit-payload", value_name = "PAYLOAD_JSON")]
    pub edit_payload: Option<String>,
}

impl VerbGroup for SafetyArgs {
    fn is_group_active(&self) -> bool {
        self.test_gate.is_some()
            || self.situ.is_some()
            || self.safe_delete.is_some()
            || self.exercises.is_some()
            || self.pr_context.is_some()
            || self.merge_scout.is_some()
            || self.dmm.is_some()
            || self.plan_lanes.is_some()
            || self.scan_skills.is_some()
            || self.edit_plan.is_some()
            || self.edit_payload.is_some()
            || self.partition.is_some()
            || self.seams
            || self.pin_census
            || self.baseline
            || self.baseline_update
    }
}

impl CliVerb for SafetyArgs {
    fn execute_verb<W: Write>(
        &self,
        ctx: &ExecutionContext<'_>,
        out: &mut crate::serialize::XmlWriter<'_, W>,
    ) -> io::Result<()> {
        let page = crate::serialize::PageRequest {
            limit: ctx.cli.modifiers.limit,
            offset: ctx.cli.modifiers.offset,
        };
        let safety = crate::query::safety::SafetyContext {
            root: ctx.root,
            ingest: ctx.ingest,
            graph: ctx.graph,
            interner: ctx.interner,
        };
        let res = execute_safety_verb(self, page, &safety, out);
        let code = res?;
        out.finish()?;
        if let Some(code) = code {
            std::process::exit(code);
        }
        Ok(())
    }
}

/// Route a top-level safety verb; `--test-gate` pages its untested rows with `page`.
pub fn execute_safety_verb<W: Write>(
    args: &SafetyArgs,
    page: crate::serialize::PageRequest,
    ctx: &crate::query::safety::SafetyContext<'_>,
    out: &mut W,
) -> io::Result<Option<i32>> {
    let Some(files_opt) = &args.test_gate else {
        return execute_safety_dispatch(args, ctx, out);
    };
    let failed = execute_test_gate_query_paged(
        ctx.root,
        files_opt.as_deref(),
        page,
        ctx.ingest,
        ctx.graph,
        ctx.interner,
        out,
    )?;
    Ok(failed.then_some(4))
}

/// Decoded safety flags; defaults when the command line was not parsed with this plugin.
#[must_use]
pub fn args(cli: &Cli) -> SafetyArgs {
    cli.plugins.decode::<SafetyArgs>().unwrap_or_default()
}

pub struct Safety;

pub static SAFETY: Safety = Safety;

impl Command for Safety {
    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        SafetyArgs::augment_args(cmd)
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let args = cli.plugins.decode::<SafetyArgs>()?;
        run_cli_verb(&args, cli, root)
    }

    fn tools(&self) -> &'static [&'static CommandDescriptor] {
        tools::TOOLS
    }
}
