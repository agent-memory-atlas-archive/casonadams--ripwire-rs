//! Detail ladder: `--expand`, `--outline`, `--pack-top-n`, `--tree`, `--biggest-first`, `--nonlocal-state`,
//! lint, naming, `--report` and `--legend-dict`.
mod ladder;

pub use ladder::execute_expand_or_outline_query;

use super::Command;
use crate::cli::{
    Cli, CliVerb, ExecutionContext, Paginated, VerbGroup, run_cli_verb, split_csv_patterns,
};
use clap::{Args, Command as ClapCommand};
use std::io::{self, Write};

/// Top-level detail ladder flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct DetailArgs {
    /// Expand full symbol bodies and callee signatures (SYM,SYM...)
    #[arg(long, value_name = "SYMS")]
    pub expand: Option<String>,
    #[arg(long, value_name = "SYMS")]
    pub outline: Option<String>,
    /// Pack bodies of the top N highest-ranked symbols
    #[arg(long = "pack-top-n", value_name = "N")]
    pub pack_top_n: Option<usize>,
    /// Orient file by file showing each file's top symbols
    #[arg(long)]
    pub tree: bool,
    /// Rank functions by readability volume and size, largest first
    #[arg(long = "biggest-first")]
    pub biggest_first: bool,
    /// Reachable nonlocal mutable state read or written per function
    #[arg(long = "nonlocal-state")]
    pub nonlocal_state: bool,
    /// Scan codebase for AST code smells across all symbols
    #[arg(long = "lint")]
    pub lint: bool,
    /// Print built-in AST code smell catalog
    #[arg(long = "lint-catalog")]
    pub lint_catalog: bool,
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
    /// Check local-variable naming consistency
    #[arg(long = "naming-locals")]
    pub naming_locals: bool,
    /// Naming convention consistency check
    #[arg(long = "naming-consistency")]
    pub naming_consistency: bool,
    /// Calibrate naming lens rules against git rename history
    #[arg(long = "naming-calibration")]
    pub naming_calibration: bool,
    /// Generate executive markdown architecture report
    #[arg(long = "report")]
    pub report: bool,
    /// Emit full schema legend dictionary
    #[arg(long = "legend-dict")]
    pub legend_dict: bool,
}

impl VerbGroup for DetailArgs {
    fn is_group_active(&self) -> bool {
        self.expand.is_some()
            || self.outline.is_some()
            || self.pack_top_n.is_some()
            || self.tree
            || self.biggest_first
            || self.nonlocal_state
            || self.lint
            || self.lint_catalog
            || self.naming_locals
            || self.naming_consistency
            || self.naming_calibration
            || self.report
            || self.legend_dict
    }
}

impl CliVerb for DetailArgs {
    fn execute_verb<W: Write>(
        &self,
        ctx: &ExecutionContext<'_>,
        out: &mut crate::serialize::XmlWriter<'_, W>,
    ) -> io::Result<()> {
        let compress = ctx.cli.modifiers.compress;
        if let Some(syms) = self.expand.as_deref().or(self.outline.as_deref()) {
            ladder::execute_expand_or_outline_compressed(
                ctx.root,
                syms,
                self.expand.is_none(),
                compress,
                ctx.ingest,
                ctx.interner,
                out,
            )
        } else if let Some(n) = self.pack_top_n {
            ladder::execute_pack_top_n_query(ctx.root, n, ctx.ingest, ctx.pr, ctx.interner, out)
        } else if self.tree {
            use crate::serialize::XmlReport;
            let file_symbols = ladder::tree_file_symbols(ctx.ingest, ctx.pr, ctx.interner);
            crate::serialize::query_xml::TreeReport {
                root: ctx.root,
                files_unlisted: ctx.ingest.files.len().saturating_sub(file_symbols.len()),
                file_symbols: &file_symbols,
                pr_iters: ctx.pr.iterations,
                page: crate::serialize::PageRequest {
                    limit: ctx.cli.modifiers.limit,
                    offset: ctx.cli.modifiers.offset,
                },
            }
            .render_envelope(out)
        } else if self.biggest_first {
            crate::metrics::readability::execute_biggest_first_query(
                ctx.root,
                ctx.ingest,
                ctx.interner,
                out,
            )
        } else if self.nonlocal_state {
            let funcs =
                crate::metrics::nonlocal::find_nonlocal_state(ctx.ingest, ctx.graph, ctx.interner);
            crate::metrics::nonlocal::serialize_nonlocal_state_with_gauge_xml(
                ctx.root,
                &funcs,
                ctx.graph.gauges(),
                out,
            )
        } else {
            execute_extended(self, ctx, out)
        }
    }

    fn force_plain_markdown(&self) -> bool {
        self.report
    }
}

fn lint_options(detail: &DetailArgs) -> crate::metrics::lint::LintOptions {
    let patterns = |raw: Option<&str>| {
        raw.into_iter()
            .flat_map(split_csv_patterns)
            .map(str::to_owned)
            .collect()
    };
    crate::metrics::lint::LintOptions {
        select: patterns(
            detail
                .lint_select
                .as_deref()
                .or(detail.lint_rules.as_deref()),
        ),
        ignore: patterns(detail.lint_ignore.as_deref()),
        budget: detail
            .lint_max_per_rule
            .unwrap_or(crate::metrics::lint::DEFAULT_RULE_BUDGET),
    }
}

fn execute_extended<W: Write>(
    detail: &DetailArgs,
    ctx: &ExecutionContext<'_>,
    stdout: &mut W,
) -> io::Result<()> {
    let (cli, root, ingest, graph, pr, interner) = (
        ctx.cli,
        ctx.root,
        ctx.ingest,
        ctx.graph,
        ctx.pr,
        ctx.interner,
    );
    if detail.lint {
        let findings = crate::metrics::lint::run_lint_scan_in_root(
            root,
            ingest,
            interner,
            lint_options(detail),
        )
        .map_err(io::Error::other)?;
        if crate::commands::architecture::args(cli).sarif {
            crate::metrics::lint::serialize_lint_sarif(&findings, stdout)
        } else {
            let page = crate::serialize::PageRequest {
                limit: cli.limit_opt(),
                offset: cli.offset_opt(),
            };
            crate::metrics::lint::serialize_lint_xml(root, &findings, page, stdout)
        }
    } else if detail.lint_catalog {
        crate::metrics::lint::serialize_lint_catalog(stdout)
    } else if detail.naming_locals {
        crate::metrics::serialize_naming_consistency_xml(root, true, ingest, interner, stdout)
    } else if detail.naming_consistency {
        crate::metrics::serialize_naming_consistency_xml(root, false, ingest, interner, stdout)
    } else if detail.naming_calibration {
        crate::metrics::serialize_naming_calibration_xml(root, ingest, interner, stdout)
    } else if detail.report {
        crate::metrics::serialize_executive_report_md(
            root, ingest, graph, &pr.ranks, interner, stdout,
        )
    } else if detail.legend_dict {
        crate::serialize::serialize_legend_dict_xml(stdout)
    } else {
        Ok(())
    }
}

pub struct Detail;

pub static DETAIL: Detail = Detail;

impl Command for Detail {
    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        DetailArgs::augment_args(cmd)
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let args = cli.plugins.decode::<DetailArgs>()?;
        run_cli_verb(&args, cli, root)
    }
}
