//! `--seams`: cross-directory call edges lacking test coverage (paged by `--limit`/`--offset`).
use super::SharedOption;
use super::plugin::report_command;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::{XmlReport, XmlWriter};
use std::io::{self, Write};

report_command! {
    name: "seams",
    args: SeamsArgs,
    static: SEAMS,
    options: [SharedOption::Paged],
    run: report(render),
    /// Cross-directory call edges lacking test coverage
    #[arg(long = "seams")]
    seams: bool,
}

fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &SeamsArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let rep = crate::gate::seams::find_integration_seams(
        ctx.ingest,
        ctx.graph,
        ctx.interner,
        ctx.cli.page_request(),
    );
    rep.write_xml_report(ctx.root, out)
}
