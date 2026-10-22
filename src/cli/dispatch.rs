use super::args::*;
use clap::{CommandFactory, FromArgMatches};
use std::io;
pub fn run_cli_args(cli: Cli) -> io::Result<()> {
    let refresh_clone = crate::commands::refreshes_remote_checkout(&cli);
    let checkout = cli
        .path
        .as_deref()
        .filter(|p| crate::git::remote::is_remote_git_url(p))
        .map(|url| crate::git::remote::checkout_remote_root(url, refresh_clone))
        .transpose()?;
    crate::commands::validate_all(&cli)?;
    let root = checkout
        .as_deref()
        .and_then(std::path::Path::to_str)
        .or(cli.path.as_deref())
        .unwrap_or(".");
    if let Some(res) = crate::commands::run(&cli, root) {
        return res;
    }
    if let Some(res) = crate::commands::run_subcommand(&cli, root) {
        return res;
    }

    crate::commands::run_default(&cli, root)
}

/// Parse the command line, including every command plugin's flags.
pub fn parse_from<I, T>(args: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let matches = crate::commands::augment(Cli::command())
        .about(ABOUT)
        .try_get_matches_from(args)?;
    let mut cli = Cli::from_arg_matches(&matches)?;
    cli.plugins = PluginArgs::new(matches);
    Ok(cli)
}

/// Main CLI entry point.
pub fn run_cli() -> io::Result<()> {
    let args = std::env::args().map(|a| if a == "-v" { "-V".to_string() } else { a });
    let cli = parse_from(args).unwrap_or_else(|error| error.exit());
    run_cli_args(cli)
}
