//! `gate` subcommand: pre-commit verification gates scoped to a repository path.
//! The gate flags are owned by their command modules and reused here through flatten.
use super::affected::{AffectedArgs, run_affected};
use super::clones::CloneArgs;
use super::common::gate::validate_gate_modifiers;
use super::edit_check::{EditCheckArgs, run_edit_check};
use super::quality_baseline::{QualityBaselineArgs, QualityBaselineFlags, run_quality_baseline};
use super::quality_delta::{QualityDeltaArgs, QualityDeltaFlags, run_quality_delta};
use super::{Command, Run, SharedOption, decode_subcommand};
use crate::cli::Cli;
use clap::{Args, Command as ClapCommand};
use std::io;

const PAGED: &[SharedOption] = &[SharedOption::Paged];

#[derive(Args, Debug, Default, PartialEq)]
pub struct GateArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

    #[command(flatten)]
    pub edit_check: EditCheckArgs,

    #[command(flatten)]
    pub affected: AffectedArgs,

    #[command(flatten)]
    pub quality_baseline: QualityBaselineArgs,

    #[command(flatten)]
    pub quality_delta: QualityDeltaArgs,

    #[command(flatten)]
    pub clones: CloneArgs,
}

/// The gate the `gate` subcommand runs, in dispatch precedence.
#[derive(Debug, PartialEq, Eq)]
pub enum GateChoice<'a> {
    EditCheck(&'a str),
    Affected(&'a str),
    QualityBaseline,
    QualityDelta,
    Clones,
    Nothing,
}

/// Picks the gate: edit-check, affected, quality-baseline, quality-delta, then clones.
#[must_use]
pub fn select_gate<'a>(
    edit_check: Option<&'a str>,
    affected: Option<&'a str>,
    quality_baseline: bool,
    quality_delta: bool,
    clones: bool,
) -> GateChoice<'a> {
    if let Some(symbol) = edit_check {
        GateChoice::EditCheck(symbol)
    } else if let Some(specs) = affected {
        GateChoice::Affected(specs)
    } else if quality_baseline {
        GateChoice::QualityBaseline
    } else if quality_delta {
        GateChoice::QualityDelta
    } else if clones {
        GateChoice::Clones
    } else {
        GateChoice::Nothing
    }
}

fn execute_gate_subcommand(cli: &Cli, default_root: &str, args: &GateArgs) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(default_root);
    let top_edit = cli.plugins.decode::<EditCheckArgs>().unwrap_or_default();
    let top_affected = cli.plugins.decode::<AffectedArgs>().unwrap_or_default();
    let top_baseline = cli
        .plugins
        .decode::<QualityBaselineFlags>()
        .unwrap_or_default();
    let top_delta = cli.plugins.decode::<QualityDeltaFlags>().unwrap_or_default();

    let quality_baseline =
        args.quality_baseline.quality_baseline || top_baseline.primary.quality_baseline;
    let quality_delta = args.quality_delta.quality_delta
        || top_delta.primary.quality_delta
        || top_delta.opts.quality_ack.is_some();
    validate_gate_modifiers(
        top_baseline.opts.allow_dirty,
        quality_baseline,
        top_delta.opts.show_stale,
        quality_delta,
    )?;

    let run = Run::new(cli, root, PAGED);
    match select_gate(
        args.edit_check
            .edit_check
            .as_deref()
            .or(top_edit.edit_check.as_deref()),
        args.affected
            .affected
            .as_deref()
            .or(top_affected.affected.as_deref()),
        quality_baseline,
        quality_delta,
        args.clones.clones,
    ) {
        GateChoice::EditCheck(symbol) => run_edit_check(&run, symbol),
        GateChoice::Affected(specs) => run_affected(&run, specs),
        GateChoice::QualityBaseline => run_quality_baseline(&run, top_baseline.opts.allow_dirty),
        GateChoice::QualityDelta => run_quality_delta(&run, &top_delta.opts),
        GateChoice::Clones => {
            crate::clones::execute_clones_command(root, args.clones.min_tokens, run.page())
        }
        GateChoice::Nothing => Ok(()),
    }
}

pub struct Gate;

pub static GATE: Gate = Gate;

impl Command for Gate {
    fn subcommands(&self) -> Vec<ClapCommand> {
        vec![
            GateArgs::augment_args(ClapCommand::new("gate"))
                .mut_arg("quality_delta", |arg| {
                    arg.help("Measure quality delta against baseline")
                })
                .about("Pre-commit verification gates (edit-check, affected, quality-delta)"),
        ]
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
        Some(decode_subcommand::<GateArgs>(matches).and_then(|args| {
            execute_gate_subcommand(cli, root, &args)
        }))
    }
}
