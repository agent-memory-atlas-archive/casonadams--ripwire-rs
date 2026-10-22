//! `--biggest-first`: functions ranked by readability volume and size, largest first.
use super::plugin::report_command;
use super::SharedOption;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "biggest-first",
    args: BiggestFirstArgs,
    static: BIGGEST_FIRST,
    options: [SharedOption::Paged],
    run: report(render),
    /// Rank functions by readability volume and size, largest first
    #[arg(long = "biggest-first")]
    biggest_first: bool,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &BiggestFirstArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    crate::metrics::execute_biggest_first_query(
        ctx.root,
        ctx.ingest,
        ctx.interner,
        ctx.cli.page_request(),
        out,
    )
}
