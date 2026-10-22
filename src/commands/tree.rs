//! `--tree`: one row per file with symbols, files ordered by their best-ranked symbol.
use super::SharedOption;
use super::plugin::report_command;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::domain::{CorpusView, IngestResult, Interner};
use crate::rank::PageRankResult;
use crate::serialize::query_xml::TreeReport;
use crate::serialize::{TreeFileSymbols, XmlReport, XmlWriter};
use std::io::{self, Write};

report_command! {
    name: "tree",
    args: TreeArgs,
    static: TREE,
    options: [SharedOption::Paged],
    run: report(render),
    /// Orient file by file showing each file's top symbols
    #[arg(long)]
    tree: bool,
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

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &TreeArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let file_symbols = tree_file_symbols(ctx.ingest, ctx.pr, ctx.interner);
    TreeReport {
        root: ctx.root,
        files_unlisted: ctx.ingest.files.len().saturating_sub(file_symbols.len()),
        file_symbols: &file_symbols,
        pr_iters: ctx.pr.iterations,
        page: ctx.cli.page_request(),
    }
    .render_envelope(out)
}
