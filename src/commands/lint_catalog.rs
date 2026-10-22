//! `--lint-catalog`: the built-in AST code smell rule catalog.
use super::plugin::report_command;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "lint-catalog",
    args: LintCatalogArgs,
    static: LINT_CATALOG,
    run: report(render),
    /// Print built-in AST code smell catalog
    #[arg(long = "lint-catalog")]
    lint_catalog: bool,
}

pub fn render<W: Write>(
    _ctx: &ExecutionContext<'_>,
    _args: &LintCatalogArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    crate::metrics::lint::serialize_lint_catalog(out)
}
