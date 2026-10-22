//! `orient` subcommand: repository orientation, optionally ranked for a task.
use crate::cli::Cli;
use crate::commands::map::dispatch_default_run;
use crate::commands::context::{OutputDialect, load_repo_analysis_for_cli};
use clap::Args;
use std::io;
use std::path::Path;

/// `orient` subcommand: repository orientation with an optional task description.
#[derive(Args, Debug, Default, PartialEq)]
pub struct OrientArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,
    /// Task description for relevance-ranked orientation
    #[arg(long = "for", value_name = "TASK")]
    pub for_task: Option<String>,
}

pub fn execute_orient_subcommand(cli: &Cli, args: &OrientArgs, default_root: &str) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(default_root);
    let Some(task) = args.for_task.as_deref() else {
        return dispatch_default_run(cli, root);
    };
    let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
    let sources = crate::query::FileSources {
        root: Path::new(root),
    };
    let at = crate::git::history::compute_git_at_stamp(root);
    let query = crate::query::commands::QueryContext {
        root,
        ingest: &ingest,
        graph: &graph,
        pr: &pr,
        interner: &interner,
        sources: &sources,
        commits: None,
        at: at.as_deref(),
    };
    let val = crate::commands::retrieval::orient_for_task(cli, &query, task);
    if let Some(error) = val["error"].as_str() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, error));
    }
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, false);
    crate::serialize::serialize_for_xml(&val, &mut stdout)?;
    stdout.finish()
}

