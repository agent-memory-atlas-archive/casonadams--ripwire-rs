//! Clones group: `--clones` and `--min-tokens`. The detector lives in `crate::clones`.

use super::Command;
use crate::cli::Cli;
use clap::{Args, Command as ClapCommand};
use std::io;

/// Top-level AST clone detection flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct CloneFlags {
    /// Detect AST code duplication and clones (Type-1, Type-2, Type-3)
    #[arg(long)]
    pub clones: bool,

    /// Minimum token length for clone detection
    #[arg(long, default_value_t = 40)]
    pub min_tokens: usize,
}

pub struct Clones;

pub static CLONES: Clones = Clones;

impl Command for Clones {
    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        CloneFlags::augment_args(cmd)
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let flags = cli.plugins.decode::<CloneFlags>()?;
        flags
            .clones
            .then(|| crate::clones::execute_clones_command(root, flags.min_tokens))
    }
}
