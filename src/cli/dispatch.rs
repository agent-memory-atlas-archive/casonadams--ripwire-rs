use super::args::*;
use crate::commands::context::{OutputDialect, load_repo_analysis_for_cli};
use crate::query::map::*;
use clap::{CommandFactory, FromArgMatches};
use std::io;
pub fn dispatch_default_run(cli: &Cli, root: &str, budget: Option<usize>) -> io::Result<()> {
    let emit_legend = !cli.is_legend_none();
    let legend_full = cli.legend.eq_ignore_ascii_case("full");
    let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, false);
    let rank_by = if cli.modifiers.stable || cli.modifiers.order.eq_ignore_ascii_case("stable") {
        "stable"
    } else {
        &cli.rank_by
    };
    let map_diff = crate::commands::safety::args(cli).map_diff;
    let changed_files = map_diff.then(|| crate::git::repo::working_tree_changes(root));
    let at = map_diff.then(|| {
        crate::git::history::compute_git_at_stamp(root).unwrap_or_else(|| "HEAD".to_string())
    });
    let mut args = MapRequest::new(root);
    args.rank_by = rank_by;
    args.pack_budget = budget;
    args.top_k = cli.top_k.or(args.top_k);
    args.emit_legend = emit_legend;
    args.legend_full = legend_full;
    args.map_diff = changed_files
        .as_ref()
        .zip(at.as_deref())
        .map(|(files, at)| MapDiff { files, at });
    args.no_inventory = cli.modifiers.no_inventory;
    execute_stream_xml_ranked(&args, &ingest, &graph, &pr, &interner, &mut stdout)?;
    stdout.finish()
}
pub fn run_cli_args(cli: Cli) -> io::Result<()> {
    let refresh_clone = crate::commands::retrieval::refresh_clone_requested(&cli);
    let checkout = cli
        .path
        .as_deref()
        .filter(|p| crate::git::remote::is_remote_git_url(p))
        .map(|url| crate::git::remote::checkout_remote_root(url, refresh_clone))
        .transpose()?;
    crate::commands::architecture::validate_modifiers(&cli)?;
    crate::commands::retrieval::validate_modifiers(&cli)?;
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

    let budget = cli.effective_budget_bytes();
    dispatch_default_run(&cli, root, budget)
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
