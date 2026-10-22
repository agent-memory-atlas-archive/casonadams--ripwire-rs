//! Command plugins: each module under `src/commands/<name>/` owns the CLI flags, CLI handler and MCP tools
//! of one command group. Shared code only sees the [`Command`] trait; adding a command is a new module
//! plus one line in `register_commands!`.
use crate::cli::Cli;
use crate::query::commands::CommandDescriptor;

pub trait Command: Sync {
    /// Add this command's clap flags (a derive `Args::augment_args`) to the CLI.
    fn augment(&self, cmd: clap::Command) -> clap::Command {
        cmd
    }

    /// Run the command when its flags are present on the command line; `None` when they are not.
    fn run(&self, _cli: &Cli, _root: &str) -> Option<std::io::Result<()>> {
        None
    }

    /// MCP tool descriptors this command contributes.
    fn tools(&self) -> &'static [&'static CommandDescriptor] {
        &[]
    }
}

/// One line per command plugin; order is CLI dispatch precedence.
macro_rules! register_commands {
    ($($module:ident :: $instance:ident,)*) => {
        $(pub mod $module;)*

        static REGISTRY: &[&dyn Command] = &[$(&$module::$instance),*];
    };
}

register_commands! {
    nav::NAV,
    retrieval::RETRIEVAL,
    detail::DETAIL,
}

/// The CLI with every plugin's flags added.
#[must_use]
pub fn augment(cmd: clap::Command) -> clap::Command {
    REGISTRY
        .iter()
        .fold(cmd, |cmd, command| command.augment(cmd))
}

/// Run the first plugin whose flags are present.
pub fn run(cli: &Cli, root: &str) -> Option<std::io::Result<()>> {
    REGISTRY.iter().find_map(|command| command.run(cli, root))
}

/// Every plugin's MCP tools.
pub fn tools() -> impl Iterator<Item = &'static CommandDescriptor> {
    REGISTRY
        .iter()
        .flat_map(|command| command.tools().iter().copied())
}
