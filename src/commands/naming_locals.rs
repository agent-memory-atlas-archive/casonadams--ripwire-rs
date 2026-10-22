//! `--naming-locals`: local-variable naming consistency check.
use super::SharedOption;
use super::plugin::report_command;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "naming-locals",
    args: NamingLocalsArgs,
    static: NAMING_LOCALS,
    options: [SharedOption::Paged],
    run: report(render),
    /// Check local-variable naming consistency
    #[arg(long = "naming-locals")]
    naming_locals: bool,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &NamingLocalsArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    crate::metrics::serialize_naming_consistency_xml(
        crate::metrics::NamingScope { root: ctx.root, include_locals: true },
        ctx.ingest,
        ctx.interner,
        ctx.cli.page_request(),
        out,
    )
}
