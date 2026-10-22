//! `--lint-catalog`: the built-in AST code smell rule catalog.
use super::{Plugin, Run};
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct LintCatalogArgs {
    /// Print built-in AST code smell catalog
    #[arg(long = "lint-catalog")]
    pub lint_catalog: bool,
}

pub fn render(out: &mut XmlWriter<'_, impl Write>) -> io::Result<()> {
    crate::metrics::lint::serialize_lint_catalog(out)
}

fn run(run: &Run<'_>, _args: &LintCatalogArgs) -> io::Result<()> {
    run.report(|_, out| render(out))
}

pub static LINT_CATALOG: Plugin<LintCatalogArgs> = Plugin {
    name: "lint-catalog",
    options: &[],
    selected: |args| args.lint_catalog,
    run,
    check: None,
    tools: &[],
};
