//! `--tree`: one row per file with symbols, files ordered by their best-ranked symbol.
use super::{Plugin, Run, SharedOption};
use crate::commands::context::ExecutionContext;
use crate::domain::{CorpusView, IngestResult, Interner};
use crate::rank::PageRankResult;
use crate::serialize::query_xml::TreeReport;
use crate::serialize::{PageRequest, TreeFileSymbols, XmlReport, XmlWriter};
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TreeArgs {
    /// Orient file by file showing each file's top symbols
    #[arg(long)]
    pub tree: bool,
}

#[must_use]
fn tree_file_symbols<'a>(
    ingest: &'a IngestResult,
    pr: &PageRankResult,
    interner: &'a Interner,
) -> Vec<TreeFileSymbols<'a>> {
    let (file_order, buckets) = crate::serialize::bucket_symbols_by_rank(
        &ingest.symbols,
        ingest.files.len(),
        &pr.ranks,
        None,
    );
    let view = (ingest, interner);
    file_order
        .into_iter()
        .map(|f| {
            let bucket = &buckets[f as usize];
            TreeFileSymbols {
                path: view.file_path(f),
                total_symbols: bucket.len(),
                top_symbols: bucket
                    .iter()
                    .take(3)
                    .map(|&id| (ingest[id].kind.tag(), view.sym_name(&ingest[id])))
                    .collect(),
            }
        })
        .collect()
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    page: PageRequest,
    out: &mut XmlWriter<'_, impl Write>,
) -> io::Result<()> {
    let file_symbols = tree_file_symbols(ctx.ingest, ctx.pr, ctx.interner);
    TreeReport {
        root: ctx.root,
        files_unlisted: ctx.ingest.files.len().saturating_sub(file_symbols.len()),
        file_symbols: &file_symbols,
        pr_iters: ctx.pr.iterations,
        page,
    }
    .render_envelope(out)
}

fn run(run: &Run<'_>, _args: &TreeArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, run.page(), out))
}

pub static TREE: Plugin<TreeArgs> = Plugin {
    name: "tree",
    options: &[SharedOption::Paged],
    selected: |args| args.tree,
    run,
    check: None,
    tools: &[],
};
