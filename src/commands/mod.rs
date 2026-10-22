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

    /// Subcommands this command contributes to the CLI.
    fn subcommands(&self) -> Vec<clap::Command> {
        Vec::new()
    }

    /// Run the command when its flags are present on the command line; `None` when they are not.
    fn run(&self, _cli: &Cli, _root: &str) -> Option<std::io::Result<()>> {
        None
    }

    /// Run subcommand `name` (its own parsed `matches`) when it was selected; `None` otherwise.
    fn run_subcommand(
        &self,
        _cli: &Cli,
        _name: &str,
        _matches: &clap::ArgMatches,
        _root: &str,
    ) -> Option<std::io::Result<()>> {
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
    workspace::WORKSPACE,
    detail::DETAIL,
    safety::SAFETY,
    architecture::ARCHITECTURE,
    metrics::METRICS,
    gate::GATE,
    clones::CLONES,
    html::HTML,
    slice::SLICE,
}

/// The CLI with every plugin's flags and subcommands added.
#[must_use]
pub fn augment(cmd: clap::Command) -> clap::Command {
    REGISTRY.iter().fold(cmd, |cmd, command| {
        command
            .subcommands()
            .into_iter()
            .fold(command.augment(cmd), |cmd, sub| cmd.subcommand(sub))
    })
}

/// Run the first plugin whose flags are present.
pub fn run(cli: &Cli, root: &str) -> Option<std::io::Result<()>> {
    REGISTRY.iter().find_map(|command| command.run(cli, root))
}

/// Run the plugin subcommand the command line selected, if a plugin owns it.
pub fn run_subcommand(cli: &Cli, root: &str) -> Option<std::io::Result<()>> {
    let (name, matches) = cli.plugins.subcommand()?;
    REGISTRY
        .iter()
        .find_map(|command| command.run_subcommand(cli, name, matches, root))
}

/// Decode a plugin subcommand's own `Args` from the matches clap produced for it.
pub fn decode_subcommand<A: clap::FromArgMatches>(
    matches: &clap::ArgMatches,
) -> std::io::Result<A> {
    A::from_arg_matches(matches).map_err(std::io::Error::other)
}

/// Every plugin's MCP tools.
pub fn tools() -> impl Iterator<Item = &'static CommandDescriptor> {
    REGISTRY
        .iter()
        .flat_map(|command| command.tools().iter().copied())
}
