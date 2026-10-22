//! `--html [FILE]` / `--color-by`: writes the interactive call-graph HTML page.
use super::{Plugin, Run};
use crate::cli::Cli;
use crate::commands::context::{Paginated, load_repo_analysis_for_cli};
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct HtmlArgs {
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
    let page = cli.page_request();
    if let Some(path) = out_file {
        let mut file = std::fs::File::create(path)?;
        crate::serialize::html::write_call_graph_html(
            &mut file, &ingest, &graph, &pr.ranks, &interner, color_by, page,
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
            page,
        )
    }
}

fn run(run: &Run<'_>, args: &HtmlArgs) -> io::Result<()> {
    let out_file = args.html.as_ref().and_then(|file| file.as_deref());
    execute_html_export(run.cli, run.root, out_file, &args.color_by)
}

pub static HTML: Plugin<HtmlArgs> = Plugin {
    name: "html",
    options: &[],
    selected: |args| args.html.is_some(),
    run,
    check: None,
    tools: &[],
};
