//! `--deps`: file-to-file dependency graph with instability and cycle metrics; honors `--limit` / `--offset`.
use super::plugin::report_command;
use super::SharedOption;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::metrics::architecture::compute_deps_report;
use crate::serialize::{PageRequest, XmlWriter};
use std::io::{self, Write};

report_command! {
    name: "deps",
    args: DepsArgs,
    static: DEPS,
    options: [SharedOption::Paged],
    run: report(render),
    /// File-to-file dependency graph with instability and cycle metrics
    #[arg(long)]
    deps: bool,
}

pub fn execute_deps_query_paged<W: Write>(
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    page: PageRequest,
    out: &mut W,
) -> io::Result<()> {
    let mut rep = compute_deps_report(ingest, graph, interner);
    rep.page = page;
    crate::serialize::serialize_deps_report_xml(root, &rep, out)
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &DepsArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    execute_deps_query_paged(
        ctx.root,
        ctx.ingest,
        ctx.graph,
        ctx.interner,
        ctx.cli.page_request(),
        out,
    )
}
