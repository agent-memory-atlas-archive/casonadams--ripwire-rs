//! Html group: `--html [FILE]` and `--color-by`, which write the interactive call-graph page.

use super::Command;
use crate::cli::{Cli, load_repo_analysis_for_cli};
use clap::{Args, Command as ClapCommand};
use std::io;

/// Top-level call-graph HTML export flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct HtmlFlags {
    #[arg(long, value_name = "FILE")]
    pub html: Option<Option<String>>,

    /// Node coloring mode for --html: lang, community, cx, churn, tested
    #[arg(long, default_value = "lang")]
    pub color_by: String,
}

fn execute_html_export(
    cli: &Cli,
    root: &str,
    out_file: Option<&str>,
    color_by: &str,
) -> io::Result<()> {
    let color_by = color_by.parse().unwrap_or_default();
    let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
    if let Some(path) = out_file {
        let mut file = std::fs::File::create(path)?;
        crate::serialize::html::write_call_graph_html(
            &mut file, &ingest, &graph, &pr.ranks, &interner, color_by, None,
        )
    } else {
        let mut stdout = io::stdout().lock();
        crate::serialize::html::write_call_graph_html(
            &mut stdout,
            &ingest,
            &graph,
            &pr.ranks,
            &interner,
            color_by,
            None,
        )
    }
}

pub struct Html;

pub static HTML: Html = Html;

impl Command for Html {
    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        HtmlFlags::augment_args(cmd)
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let flags = cli.plugins.decode::<HtmlFlags>()?;
        let out_file = flags.html.as_ref()?;
        Some(execute_html_export(
            cli,
            root,
            out_file.as_deref(),
            &flags.color_by,
        ))
    }
}
