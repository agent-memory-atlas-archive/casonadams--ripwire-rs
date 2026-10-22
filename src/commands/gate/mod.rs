//! Gate group: `--edit-check`, `--affected`, `--quality-baseline`, `--quality-delta` and the `gate`
//! subcommand. The gate handlers themselves live with the safety group.

use super::{Command, decode_subcommand};
use crate::cli::{Cli, Paginated};
use crate::commands::safety::{GateDispatchArgs, SafetyArgs, dispatch_gate_command};
use clap::{Args, Command as ClapCommand};
use std::io;

/// Top-level pre-commit verification flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct GateFlags {
    /// Verify 1-hop caller compatibility against git HEAD
    #[arg(long)]
    pub edit_check: Option<String>,

    /// Pin quality baseline stamped with git HEAD
    #[arg(long)]
    pub quality_baseline: bool,

    /// Report code quality delta regressions vs baseline
    #[arg(long)]
    pub quality_delta: bool,

    /// Show test files affected by changes
    #[arg(long)]
    pub affected: Option<String>,
}

/// `gate` subcommand: the same gates plus clone detection, scoped to a repository path.
#[derive(Args, Debug, Default, PartialEq)]
pub struct GateArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

    /// Verify 1-hop caller compatibility against git HEAD
    #[arg(long)]
    pub edit_check: Option<String>,

    /// Pin quality baseline stamped with git HEAD
    #[arg(long)]
    pub quality_baseline: bool,

    /// Measure quality delta against baseline
    #[arg(long)]
    pub quality_delta: bool,

    /// Show test files affected by changes
    #[arg(long)]
    pub affected: Option<String>,

    /// Detect AST code duplication and clones (Type-1, Type-2, Type-3)
    #[arg(long)]
    pub clones: bool,

    /// Minimum token length for clone detection
    #[arg(long, default_value_t = 40)]
    pub min_tokens: usize,
}

/// Decoded gate flags; defaults when the command line was not parsed with this plugin.
#[must_use]
pub fn gate_flags(cli: &Cli) -> GateFlags {
    cli.plugins.decode::<GateFlags>().unwrap_or_default()
}

fn gate_args<'a>(
    cli: &'a Cli,
    flags: &'a GateFlags,
    safety: &'a SafetyArgs,
) -> GateDispatchArgs<'a> {
    GateDispatchArgs {
        edit_check: flags.edit_check.as_deref(),
        affected: flags.affected.as_deref(),
        quality_baseline: flags.quality_baseline,
        quality_delta: flags.quality_delta || safety.quality_ack.is_some(),
        scope: safety.scope.as_deref(),
        quality_ack: safety
            .quality_ack
            .as_ref()
            .map(|o| o.as_deref().unwrap_or("acknowledged")),
        ack_only: safety.ack_only.as_deref(),
        show_stale: safety.show_stale,
        allow_dirty: safety.allow_dirty,
        limit: cli.limit_opt(),
        offset: cli.offset_opt(),
    }
}

fn execute_gate_subcommand(args: &GateArgs, cli: &Cli, default_root: &str) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(default_root);
    let flags = gate_flags(cli);
    let safety = crate::commands::safety::args(cli);
    let inherited = gate_args(cli, &flags, &safety);
    let gate = GateDispatchArgs {
        edit_check: args.edit_check.as_deref().or(inherited.edit_check),
        affected: args.affected.as_deref().or(inherited.affected),
        quality_baseline: args.quality_baseline || inherited.quality_baseline,
        quality_delta: args.quality_delta || inherited.quality_delta,
        ..inherited
    };
    if let Some(result) = dispatch_gate_command(root, gate, cli) {
        return result;
    }
    if args.clones {
        return crate::clones::execute_clones_command(root, args.min_tokens);
    }
    Ok(())
}

pub struct Gate;

pub static GATE: Gate = Gate;

impl Command for Gate {
    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        GateFlags::augment_args(cmd)
    }

    fn subcommands(&self) -> Vec<ClapCommand> {
        vec![
            GateArgs::augment_args(ClapCommand::new("gate"))
                .about("Pre-commit verification gates (edit-check, affected, quality-delta)"),
        ]
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        if matches!(cli.plugins.subcommand(), Some(("gate", _))) {
            return None;
        }
        let flags = gate_flags(cli);
        let safety = crate::commands::safety::args(cli);
        dispatch_gate_command(root, gate_args(cli, &flags, &safety), cli)
    }

    fn run_subcommand(
        &self,
        cli: &Cli,
        name: &str,
        matches: &clap::ArgMatches,
        root: &str,
    ) -> Option<io::Result<()>> {
        if name != "gate" {
            return None;
        }
        Some(
            decode_subcommand::<GateArgs>(matches)
                .and_then(|args| execute_gate_subcommand(&args, cli, root)),
        )
    }
}
