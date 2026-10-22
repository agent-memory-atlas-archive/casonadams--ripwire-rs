//! `--lint`: AST code smell scan; `--sarif` picks the export format, paging applies to the XML path.
use super::{Plugin, Run, SharedOption};
use crate::commands::context::{ExecutionContext, split_csv_patterns};
use crate::serialize::{PageRequest, XmlWriter};
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct LintArgs {
    /// Scan codebase for AST code smells across all symbols
    #[arg(long = "lint")]
    pub lint: bool,
    /// SARIF format export for lint
    #[arg(long = "sarif")]
    pub sarif: bool,
    /// External YAML ast-grep lint rules directory
    #[arg(long = "lint-rules", value_name = "DIR")]
    pub lint_rules: Option<String>,
    /// Select specific lint rules to run
    #[arg(long = "lint-select", value_name = "RULES")]
    pub lint_select: Option<String>,
    /// Ignore specific lint rules
    #[arg(long = "lint-ignore", value_name = "RULES")]
    pub lint_ignore: Option<String>,
    /// Maximum findings emitted per lint rule
    #[arg(long = "lint-max-per-rule", value_name = "N")]
    pub lint_max_per_rule: Option<usize>,
}

fn lint_options(args: &LintArgs) -> crate::metrics::lint::LintOptions {
    let patterns = |raw: Option<&str>| {
        raw.into_iter()
            .flat_map(split_csv_patterns)
            .map(str::to_owned)
            .collect()
    };
    crate::metrics::lint::LintOptions {
        select: patterns(
            args.lint_select
                .as_deref()
                .or(args.lint_rules.as_deref()),
        ),
        ignore: patterns(args.lint_ignore.as_deref()),
        budget: args
            .lint_max_per_rule
            .unwrap_or(crate::metrics::lint::DEFAULT_RULE_BUDGET),
    }
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    args: &LintArgs,
    page: PageRequest,
    out: &mut XmlWriter<'_, impl Write>,
) -> io::Result<()> {
    let findings = crate::metrics::lint::run_lint_scan_in_root(
        ctx.root,
        ctx.ingest,
        ctx.interner,
        lint_options(args),
    )
    .map_err(io::Error::other)?;
    if args.sarif {
        crate::metrics::lint::serialize_lint_sarif(&findings, out)
    } else {
        crate::metrics::lint::serialize_lint_xml(ctx.root, &findings, page, out)
    }
}

fn run(run: &Run<'_>, args: &LintArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, run.page(), out))
}

pub static LINT: Plugin<LintArgs> = Plugin {
    name: "lint",
    options: &[SharedOption::Paged],
    selected: |args| args.lint,
    run,
    check: None,
    tools: &[],
};
