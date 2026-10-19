use super::args::*;
use super::queries::*;
use super::safety::*;
use super::stream::*;
use crate::domain::Interner;
use crate::graph::Graph;
use crate::mcp::run_mcp_stdio;
use crate::rank::PageRankResult;
use clap::Parser;
use std::io::{self, Write};
use std::path::Path;
pub fn execute_navigate_with<W: Write>(
    args: &NavigateArgs,
    ctx: &NavContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    let Some(sym_name) = args.symbol.as_deref() else {
        return writeln!(out, "Error: --symbol is required for navigate commands");
    };
    let ids = crate::navigate::find_symbols_by_name(sym_name, ctx.ingest, ctx.interner);
    if ids.is_empty() {
        return writeln!(out, "Symbol '{sym_name}' not found");
    }
    if args.impact {
        use crate::domain::CorpusView;
        let impact = crate::navigate::transitive_impact_multi(
            &ids,
            ctx.ingest,
            ctx.graph,
            ctx.ranks,
            ctx.interner,
            None,
        );
        let importers = (ctx.ingest, ctx.interner).resolve_symbol_importers(&ids);
        return crate::navigate::serialize_impact_with_importers_xml(
            ".",
            sym_name,
            &impact,
            importers,
            &ctx.graph.xml_gauge_attrs(),
            1,
            out,
        );
    }
    if (args.callers || args.callees)
        && let Some(nav) = crate::navigate::navigate_1hop_multi(
            &ids,
            ctx.ingest,
            ctx.graph,
            ctx.ranks,
            ctx.interner,
        )
    {
        return if args.callers {
            crate::navigate::serialize_callers_xml(sym_name, &nav.callers, out)
        } else {
            crate::navigate::serialize_callees_xml(sym_name, &nav.callees, out)
        };
    }
    if let Some(nav) =
        crate::navigate::navigate_1hop(ids[0], ctx.ingest, ctx.graph, ctx.ranks, ctx.interner)
    {
        crate::navigate::serialize_find_symbol_xml(&nav, out)?;
    }
    Ok(())
}

pub fn execute_navigate(args: &NavigateArgs) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(".");
    let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
    let ctx = NavContext {
        ingest: &ingest,
        graph: &graph,
        ranks: &pr.ranks,
        interner: &interner,
    };
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
    execute_navigate_with(args, &ctx, &mut stdout)?;
    stdout.flush()
}

pub fn execute_html_export(
    root: &str,
    out_file: Option<&str>,
    color_by_str: &str,
) -> io::Result<()> {
    let color_by = color_by_str.parse().unwrap_or_default();
    let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
    if let Some(path) = out_file {
        let mut file = std::fs::File::create(path)?;
        crate::serialize::html::write_call_graph_html(
            &mut file, &ingest, &graph, &pr.ranks, &interner, color_by, None,
        )?;
    } else {
        let mut stdout = io::stdout().lock();
        crate::serialize::html::write_call_graph_html(
            &mut stdout,
            &ingest,
            &graph,
            &pr.ranks,
            &interner,
            color_by,
            None,
        )?;
    }
    Ok(())
}

pub fn execute_stream_xml(root: &str) -> io::Result<()> {
    let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
    execute_stream_xml_with(root, &ingest, &graph, &pr, &interner, &mut stdout)?;
    stdout.flush()
}

pub fn load_repo_analysis(
    root: &str,
) -> io::Result<(crate::domain::IngestResult, Graph, PageRankResult, Interner)> {
    load_repo_analysis_cached(root, None, false)
}

pub fn load_repo_analysis_cached(
    root: &str,
    cache_path: Option<&Path>,
    no_cache: bool,
) -> io::Result<(crate::domain::IngestResult, Graph, PageRankResult, Interner)> {
    let opts = crate::crawler::CrawlerOptions::default();
    load_repo_analysis_with_options(root, cache_path, no_cache, &opts)
}

pub fn load_repo_analysis_with_options(
    root: &str,
    cache_path: Option<&Path>,
    no_cache: bool,
    opts: &crate::crawler::CrawlerOptions,
) -> io::Result<(crate::domain::IngestResult, Graph, PageRankResult, Interner)> {
    let default_opts = *opts == crate::crawler::CrawlerOptions::default();
    if !no_cache
        && default_opts
        && let Some(cached) = crate::cache::load_cached_analysis(root, cache_path)
    {
        return Ok(cached);
    }
    let files = crate::crawler::crawl_directory(root, opts);
    let registry =
        crate::ingest::LanguageRegistry::new().map_err(|e| io::Error::other(e.to_string()))?;
    let interner = Interner::new();
    let ingest = crate::ingest::ingest_crawled_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&crate::rank::PageRankConfig::default());
    if default_opts {
        let _ =
            crate::cache::save_cached_analysis(root, cache_path, (&ingest, &graph, &pr, &interner));
    }
    Ok((ingest, graph, pr, interner))
}

#[must_use]
pub fn crawler_options_for_cli(cli: &Cli) -> crate::crawler::CrawlerOptions {
    let defaults = crate::crawler::CrawlerOptions::default();
    crate::crawler::CrawlerOptions {
        max_file_size: cli
            .modifiers
            .max_file_size
            .unwrap_or(defaults.max_file_size),
        honor_gitignore: !cli.modifiers.no_ignore,
        ignore_tests: cli.modifiers.ignore_tests,
        custom_excludes: cli
            .modifiers
            .exclude
            .as_deref()
            .map(|s| {
                split_csv_patterns(s)
                    .into_iter()
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub fn load_repo_analysis_for_cli(
    cli: &Cli,
    root: &str,
) -> io::Result<(crate::domain::IngestResult, Graph, PageRankResult, Interner)> {
    let cache_path = cli.modifiers.cache.as_deref().map(Path::new);
    let opts = crawler_options_for_cli(cli);
    load_repo_analysis_with_options(root, cache_path, cli.modifiers.no_cache, &opts)
}

pub fn compute_effective_budget(
    pack_budget: Option<usize>,
    token_budget: Option<usize>,
    max_tokens: Option<usize>,
) -> Option<usize> {
    if let Some(tb) = token_budget.or(max_tokens) {
        if tb <= 200 {
            Some(tb.saturating_mul(4))
        } else {
            Some(tb.saturating_mul(2).saturating_sub(1100).max(512))
        }
    } else {
        pack_budget.or(Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES))
    }
}

pub fn dispatch_top_nav(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let (sym, mode) = cli.nav.active_query()?;
    Some((|| {
        let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root)?;
        let mut stdout_lock = io::stdout().lock();
        if cli.modifiers.json {
            let ids = crate::navigate::find_symbols_by_name(sym, &ingest, &interner);
            if let Some(&id) = ids.first() {
                if mode == NavMode::Impact {
                    let impact = crate::navigate::transitive_impact(
                        id, &ingest, &graph, &pr.ranks, &interner, None,
                    );
                    serde_json::to_writer_pretty(&mut stdout_lock, &impact)?;
                    writeln!(stdout_lock)?;
                    return Ok(());
                } else if let Some(nav) =
                    crate::navigate::navigate_1hop(id, &ingest, &graph, &pr.ranks, &interner)
                {
                    serde_json::to_writer_pretty(&mut stdout_lock, &nav)?;
                    writeln!(stdout_lock)?;
                    return Ok(());
                }
            }
        }
        let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, false);
        let nav_ctx = NavContext {
            ingest: &ingest,
            graph: &graph,
            ranks: &pr.ranks,
            interner: &interner,
        };
        execute_nav_symbol_query_in_root(root, sym, mode, &nav_ctx, &mut stdout)?;
        stdout.flush()
    })())
}

/// Shared execution context bundling CLI options, repository root, and indexed analysis state.
pub struct ExecutionContext<'a> {
    pub cli: &'a Cli,
    pub root: &'a str,
    pub ingest: &'a crate::domain::IngestResult,
    pub graph: &'a Graph,
    pub pr: &'a PageRankResult,
    pub interner: &'a Interner,
}

impl<'a> crate::domain::CorpusView<'a> for ExecutionContext<'a> {
    #[inline]
    fn ingest(&self) -> &'a crate::domain::IngestResult {
        self.ingest
    }

    #[inline]
    fn interner(&self) -> &'a Interner {
        self.interner
    }
}

impl Paginated for ExecutionContext<'_> {
    #[inline]
    fn limit_opt(&self) -> Option<usize> {
        self.cli.limit_opt()
    }

    #[inline]
    fn offset_opt(&self) -> Option<usize> {
        self.cli.offset_opt()
    }
}

impl ExecutionContext<'_> {
    #[inline]
    pub fn write_report<R: crate::serialize::XmlReport, W: Write>(
        &self,
        rep: &R,
        out: &mut W,
    ) -> io::Result<bool> {
        rep.write_xml_report(self.root, out)?;
        Ok(true)
    }
}

/// Trait for CLI verb groups that execute against an `ExecutionContext`.
pub trait CliVerb: VerbGroup {
    fn execute_verb<W: Write>(&self, ctx: &ExecutionContext<'_>, out: &mut W) -> io::Result<()>;

    fn force_plain_markdown(&self) -> bool {
        false
    }
}

pub fn run_cli_verb<V: CliVerb>(verb: &V, cli: &Cli, root: &str) -> Option<io::Result<()>> {
    if !verb.is_group_active() {
        return None;
    }
    Some((|| {
        let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root)?;
        let ctx = ExecutionContext {
            cli,
            root,
            ingest: &ingest,
            graph: &graph,
            pr: &pr,
            interner: &interner,
        };
        let plain = verb.force_plain_markdown();
        let mut stdout_lock = io::stdout().lock();
        let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, plain);
        verb.execute_verb(&ctx, &mut stdout)?;
        stdout.flush()
    })())
}
impl CliVerb for TopRetrievalArgs {
    fn execute_verb<W: Write>(&self, ctx: &ExecutionContext<'_>, out: &mut W) -> io::Result<()> {
        if let Some(expr) = self.graph_query.as_deref() {
            return crate::navigate::execute_graph_query_expr_paged(
                expr,
                ctx.root,
                ctx.ingest,
                ctx.graph,
                ctx.interner,
                crate::navigate::GraphQueryPageOptions {
                    limit: ctx.cli.modifiers.limit,
                    offset: ctx.cli.modifiers.offset,
                },
                out,
            );
        }
        execute_retrieval_dispatch(
            self,
            ctx.root,
            ctx.ingest,
            ctx.graph,
            ctx.pr,
            ctx.interner,
            out,
        )
    }
}

impl CliVerb for TopDetailArgs {
    fn execute_verb<W: Write>(&self, ctx: &ExecutionContext<'_>, out: &mut W) -> io::Result<()> {
        let compress = ctx.cli.modifiers.compress;
        if let Some(syms) = self.expand.as_deref() {
            execute_expand_or_outline_compressed(
                ctx.root,
                syms,
                false,
                compress,
                ctx.ingest,
                ctx.interner,
                out,
            )
        } else if let Some(syms) = self.outline.as_deref() {
            execute_expand_or_outline_compressed(
                ctx.root,
                syms,
                true,
                compress,
                ctx.ingest,
                ctx.interner,
                out,
            )
        } else if let Some(n) = self.pack_top_n {
            execute_pack_top_n_query(ctx.root, n, ctx.ingest, ctx.pr, ctx.interner, out)
        } else if self.tree {
            execute_tree_query(ctx.root, ctx.ingest, ctx.pr, ctx.interner, out)
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
            execute_detail_extended(ctx, out)
        }
    }

    fn force_plain_markdown(&self) -> bool {
        self.report
    }
}

impl CliVerb for TopSafetyArgs {
    fn execute_verb<W: Write>(&self, ctx: &ExecutionContext<'_>, out: &mut W) -> io::Result<()> {
        let res = execute_safety_dispatch(self, ctx.root, ctx.ingest, ctx.graph, ctx.interner, out);
        out.flush()?;
        if let Some(code) = res? {
            std::process::exit(code);
        }
        Ok(())
    }
}

impl CliVerb for TopWorkspaceArgs {
    fn execute_verb<W: Write>(&self, ctx: &ExecutionContext<'_>, out: &mut W) -> io::Result<()> {
        if self.owners {
            return execute_workspace_owners(ctx, out);
        }
        if self.stray_content {
            let val = crate::mcp::doc_tools::call_stray_content(ctx.root);
            return crate::serialize::serialize_stray_content_xml(&val, out);
        }
        if let Some((spec, op)) = self.active_edit_op() {
            execute_edit_op_with_options(
                EditCliOptions {
                    root: ctx.root,
                    spec,
                    op,
                    payload_override: ctx.cli.safety.edit_payload.as_deref(),
                    dry_run: ctx.cli.safety.dry_run,
                },
                ctx.ingest,
                ctx.interner,
                out,
            )
        } else {
            execute_workspace_doc_query_paged(
                self,
                ctx.root,
                ctx.ingest,
                ctx.interner,
                ctx.cli.with_history_flag(),
                ctx.cli,
                out,
            )
        }
    }
}

fn execute_workspace_owners<W: Write>(ctx: &ExecutionContext<'_>, out: &mut W) -> io::Result<()> {
    let cutoff = crate::git::history::resolve_cutoff_timestamp(None, 12);
    let commits = crate::git::history::walk_repository_history_scoped(ctx.root, cutoff).ok();
    let at_stamp = crate::git::history::compute_git_at_stamp(ctx.root);
    let known_files: std::collections::HashSet<String> = ctx
        .ingest
        .files
        .iter()
        .map(|f| ctx.interner.resolve(&f.path).to_string())
        .collect();
    let mut val = crate::mcp::doc_tools::call_owners(
        commits.as_deref(),
        None,
        at_stamp.as_deref(),
        Some(&known_files),
    );
    val["root"] = serde_json::Value::String(ctx.root.to_string());
    ctx.apply_pagination_to_json(&mut val);
    if ctx.cli.detail_opt().is_some_and(|d| d > 0) {
        val["detail"] = serde_json::Value::Bool(true);
    }
    crate::serialize::serialize_owners_xml(&val, out)
}

fn split_csv_patterns(raw: &str) -> Vec<&str> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

fn filter_and_cap_lint_findings(
    findings: &mut Vec<crate::metrics::lint::LintFinding>,
    detail: &TopDetailArgs,
) {
    if let Some(sel) = detail
        .lint_select
        .as_deref()
        .or(detail.lint_rules.as_deref())
    {
        let pats = split_csv_patterns(sel);
        findings.retain(|f| pats.is_empty() || pats.iter().any(|&p| f.rule.contains(p)));
    }
    if let Some(ign) = detail.lint_ignore.as_deref() {
        let pats = split_csv_patterns(ign);
        findings.retain(|f| !pats.iter().any(|&p| f.rule.contains(p)));
    }
    if let Some(limit) = detail.lint_max_per_rule {
        let mut rule_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        findings.retain(|f| {
            let c = rule_counts.entry(f.rule.clone()).or_insert(0);
            *c += 1;
            *c <= limit
        });
    }
}

fn execute_detail_extended<W: Write>(ctx: &ExecutionContext<'_>, stdout: &mut W) -> io::Result<()> {
    let (cli, root, ingest, graph, pr, interner) = (
        ctx.cli,
        ctx.root,
        ctx.ingest,
        ctx.graph,
        ctx.pr,
        ctx.interner,
    );
    let detail = &cli.detail;
    if detail.lint {
        let mut findings =
            crate::metrics::lint::run_lint_scan_in_root(root, ingest, graph, interner);
        filter_and_cap_lint_findings(&mut findings, detail);
        if cli.arch.sarif {
            crate::metrics::lint::serialize_lint_sarif(&findings, stdout)
        } else {
            crate::metrics::lint::serialize_lint_xml(root, &findings, stdout)
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

fn execute_arch_rules_query<W: Write>(
    rules_file: &str,
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    stdout: &mut W,
) -> io::Result<()> {
    let content = std::fs::read_to_string(rules_file)?;
    let cfg = crate::gate::arch::ArchConfig::parse(&content);
    let violations = crate::gate::arch::check_architecture_rules(&cfg, ingest, graph, interner);
    crate::gate::arch::serialize_arch_xml(root, &violations, stdout)?;
    if !violations.is_empty() {
        std::process::exit(2);
    }
    Ok(())
}

fn execute_arch_or_zoom_query<W: Write>(
    arch_args: &TopArchitectureArgs,
    ctx: &ExecutionContext<'_>,
    out: &mut W,
) -> Option<io::Result<()>> {
    if arch_args.mermaid {
        return Some(crate::graph::generate_mermaid_diagram(
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            out,
        ));
    }
    if let Some(rules_file) = arch_args.arch.as_deref() {
        return Some(execute_arch_rules_query(
            rules_file,
            ctx.root,
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            out,
        ));
    }
    if let Some(depth) = arch_args.zoom {
        let opts = crate::graph::zoom::ZoomOptions {
            depth,
            levels_shown: arch_args.zoom_levels,
            limit: ctx.cli.modifiers.limit,
            offset: ctx.cli.modifiers.offset,
        };
        return Some(crate::graph::zoom::execute_zoom_query(
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            ctx.pr.iterations,
            opts,
            out,
        ));
    }
    None
}

impl CliVerb for TopArchitectureArgs {
    fn execute_verb<W: Write>(&self, ctx: &ExecutionContext<'_>, out: &mut W) -> io::Result<()> {
        if let Some(res) = execute_arch_or_zoom_query(self, ctx, out) {
            return res;
        }
        if self.dead_code {
            execute_dead_code_query_paged(
                ctx.root,
                ctx.ingest,
                ctx.graph,
                ctx.interner,
                ctx.cli,
                out,
            )
        } else if self.communities || self.community.is_some() {
            execute_communities_query(
                ctx.root,
                self.community,
                ctx.ingest,
                ctx.graph,
                ctx.interner,
                out,
            )
        } else if let Some(sym) = self.around.as_deref() {
            let cfg = AroundConfig {
                depth: self.around_depth,
                fanout: self.around_fanout,
            };
            execute_around_query(ctx.root, sym, cfg, ctx.ingest, ctx.graph, ctx.interner, out)
        } else if self.external_surface {
            execute_external_surface_query(ctx.root, ctx.ingest, ctx.interner, out)
        } else if self.deps {
            execute_deps_query_paged(ctx.root, ctx.ingest, ctx.graph, ctx.interner, ctx.cli, out)
        } else {
            let _ = dispatch_lens_queries(ctx, out)?;
            Ok(())
        }
    }

    fn force_plain_markdown(&self) -> bool {
        self.mermaid
    }
}

fn dispatch_lens_queries<W: Write>(ctx: &ExecutionContext<'_>, stdout: &mut W) -> io::Result<bool> {
    let (cli, root, ingest, graph, pr, interner) = (
        ctx.cli,
        ctx.root,
        ctx.ingest,
        ctx.graph,
        ctx.pr,
        ctx.interner,
    );
    let a = &cli.arch;
    if a.metrics {
        let opts = crate::serialize::SerializeOptions {
            root: root.to_string(),
            top_k: cli.top_k,
            pack_budget_bytes: if cli.top_k.is_some() {
                None
            } else {
                Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES)
            },
            metrics: true,
            ..Default::default()
        };
        crate::serialize::serialize_xml(ingest, graph, pr, interner, stdout, &opts)?;
        return Ok(true);
    }
    if let Some(preset_opt) = &a.quality_panel {
        let rep = crate::metrics::compute_quality_panel_report(
            ingest,
            graph,
            interner,
            preset_opt.as_deref(),
        );
        return ctx.write_report(&rep, stdout);
    }
    if let Some(sym) = a.layout.as_deref() {
        use crate::domain::CorpusView;
        let has_c_agg = ingest
            .symbols
            .iter()
            .any(|s| ctx.sym_name(s) == sym && s.lang.semantics().supports_c_layout());
        if !has_c_agg {
            return Err(io::Error::other(format!(
                "ripwire-rs: --layout: '{sym}' is indexed but has no C-family aggregate body — this verb models C/C++/ObjC byte layout only"
            )));
        }
        let rep = crate::metrics::compute_layout_report(sym, ingest, interner, root);
        return ctx.write_report(&rep, stdout);
    }
    if let Some(sym_opt) = &a.field_affinity {
        let rep = crate::metrics::layout::compute_field_affinity_report(
            sym_opt.as_deref(),
            ingest,
            interner,
            root,
        );
        return ctx.write_report(&rep, stdout);
    }
    if a.context_ratio {
        let rep = crate::metrics::compute_context_ratio(ingest, graph, interner);
        return ctx.write_report(&rep, stdout);
    }
    if a.ensemble {
        let rep = crate::metrics::compute_ensemble(ingest, graph, interner);
        return ctx.write_report(&rep, stdout);
    }
    if a.comment_coherence {
        let rep = crate::metrics::analysis_lenses::compute_comment_coherence_in_root(
            root, ingest, interner,
        );
        return ctx.write_report(&rep, stdout);
    }
    Ok(false)
}

pub fn dispatch_top_flags(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    if cli.modifiers.eval {
        let mut stdout = io::stdout().lock();
        return Some(crate::metrics::run_cochange_eval(&mut stdout));
    }
    if cli.modifiers.eval_retrieval {
        return Some((|| {
            let (ingest, _graph, pr, interner) = load_repo_analysis_for_cli(cli, root)?;
            let mut stdout = io::stdout().lock();
            crate::metrics::run_retrieval_eval_for_repo(&ingest, &interner, &pr.ranks, &mut stdout)
        })());
    }
    if let Some(file) = cli.modifiers.batch.as_deref() {
        return Some((|| {
            let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root)?;
            let mut stdout_lock = io::stdout().lock();
            let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, false);
            execute_batch_file(root, file, &ingest, &graph, &pr, &interner, &mut stdout)?;
            stdout.flush()
        })());
    }
    dispatch_top_nav(cli, root)
        .or_else(|| run_cli_verb(&cli.retrieval, cli, root))
        .or_else(|| run_cli_verb(&cli.workspace, cli, root))
        .or_else(|| run_cli_verb(&cli.detail, cli, root))
        .or_else(|| run_cli_verb(&cli.safety, cli, root))
        .or_else(|| run_cli_verb(&cli.arch, cli, root))
}

pub fn dispatch_default_run(cli: &Cli, root: &str, budget: Option<usize>) -> io::Result<()> {
    let emit_legend = !cli.is_legend_none();
    let legend_full = cli.legend.eq_ignore_ascii_case("full");
    let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root)?;
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, false);
    let rank_by = if cli.modifiers.stable || cli.modifiers.order.eq_ignore_ascii_case("stable") {
        "stable"
    } else {
        &cli.rank_by
    };
    let args = RankedStreamArgs {
        root,
        rank_by,
        pack_budget: budget,
        top_k: cli.top_k.or(Some(200)),
        emit_legend,
        legend_full,
        map_diff: cli.safety.map_diff,
        no_inventory: cli.modifiers.no_inventory,
    };
    execute_stream_xml_ranked(&args, &ingest, &graph, &pr, &interner, &mut stdout)?;
    stdout.flush()
}
fn execute_orient_subcommand(orient_args: &OrientArgs) -> io::Result<()> {
    let root = orient_args.path.as_deref().unwrap_or(".");
    let Some(task) = orient_args.for_task.as_deref() else {
        return execute_stream_xml(root);
    };
    let (ingest, graph, pr, interner) = load_repo_analysis_cached(root, None, false)?;
    let val = crate::mcp::retrieval::call_for_task_with_options(
        &ingest,
        &interner,
        &pr.ranks,
        task,
        crate::mcp::retrieval::ForTaskOptions {
            root,
            graph: Some(&graph),
            ..Default::default()
        },
    );
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
    crate::serialize::serialize_for_xml(&val, &mut stdout)?;
    stdout.flush()
}

pub fn dispatch_subcommand(command: &Commands, cli: &Cli, default_root: &str) -> io::Result<()> {
    match command {
        Commands::Mcp(mcp_args) => run_mcp_stdio(mcp_args.path.as_deref().unwrap_or(default_root)),
        Commands::Navigate(nav_args) => execute_navigate(nav_args),
        Commands::Orient(orient_args) => execute_orient_subcommand(orient_args),
        Commands::Gate(gate_args) => {
            let root = gate_args.path.as_deref().unwrap_or(default_root);
            let sub_gate = GateDispatchArgs {
                edit_check: gate_args.edit_check.as_deref(),
                affected: gate_args.affected.as_deref(),
                quality_baseline: gate_args.quality_baseline,
                quality_delta: gate_args.quality_delta,
                ..Default::default()
            };
            if let Some(res) = dispatch_gate_command(root, sub_gate) {
                return res;
            }
            if gate_args.clones {
                return crate::clones::execute_clones_command(root, gate_args.min_tokens);
            }
            Ok(())
        }
        Commands::Metrics(metrics_args) => {
            let root = metrics_args.path.as_deref().unwrap_or(default_root);
            if let Some(res) = dispatch_git_metrics_with_flags(
                root,
                GitMetricsFlags {
                    cochange: metrics_args.cochange.as_ref().map(|o| o.as_deref()),
                    hotspots: metrics_args.hotspots,
                    since: metrics_args.since.as_deref(),
                    ..Default::default()
                },
            ) {
                return res;
            }
            Ok(())
        }
        Commands::Slice(slice_args) => crate::slice::execute_slice_command_with_depth(
            default_root,
            slice_args.target.as_deref(),
            &slice_args.flow,
            cli.slice_depth,
        ),
        Commands::Skills(skills_args) => execute_skills_install(skills_args.path.as_deref()),
    }
}
pub fn execute_skills_install(target: Option<&str>) -> io::Result<()> {
    let report = crate::skills::install_skills(target)?;
    println!(
        "Installed {} ripwire-rs agent skills to {}",
        report.installed_count,
        report.destination.display()
    );
    println!(
        "Active manifest written to {}",
        report.destination.join(".ripwire-rs-manifest-v1").display()
    );
    for skill in &report.skills {
        println!("  - {skill}");
    }
    Ok(())
}

/// Execute CLI from parsed options.
fn dispatch_primary_workflows(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    if cli.mcp {
        return Some(crate::mcp::run_mcp_stdio_with_options(
            root,
            cli.modifiers.no_stable,
        ));
    }
    if cli.modifiers.lsp {
        return Some(crate::mcp::run_lsp_stdio(root));
    }
    if let Some(target_opt) = &cli.add_skills {
        let target = target_opt.as_deref();
        return Some(execute_skills_install(target));
    }
    if let Some(res) = dispatch_top_flags(cli, root) {
        return Some(res);
    }
    if let Some(res) = dispatch_git_metrics_with_flags(
        root,
        GitMetricsFlags {
            cochange: cli.cochange.as_ref().map(|o| o.as_deref()),
            cochange_groups: cli.safety.cochange_groups,
            cochange_recur: cli.safety.cochange_recur,
            hotspots: cli.hotspots,
            since: cli.since.as_deref(),
            pretty: cli.is_pretty_out(),
            json_out: cli.is_json_out(),
            limit: cli.limit_opt(),
            offset: cli.offset_opt(),
        },
    ) {
        return Some(res);
    }
    None
}

fn validate_arch_modifiers(a: &TopArchitectureArgs) -> io::Result<()> {
    if a.zoom_levels.is_some() && a.zoom.is_none() {
        return Err(io::Error::other(
            "ripwire-rs: --zoom-levels=N modifies --zoom; pass it too (e.g. --zoom --zoom-levels=0 prints every level)",
        ));
    }
    Ok(())
}

fn validate_retrieval_modifiers(r: &TopRetrievalArgs) -> io::Result<()> {
    let Some(raw) = r.sections.as_deref() else {
        return Ok(());
    };
    if r.task_for.is_none() {
        return Err(io::Error::other(
            "ripwire-rs: --sections= modifies --for=TASK; pass it too",
        ));
    }
    crate::mcp::retrieval::parse_for_sections(raw)
        .map(|_| ())
        .map_err(io::Error::other)
}

pub fn run_cli_args(cli: Cli) -> io::Result<()> {
    validate_arch_modifiers(&cli.arch)?;
    validate_retrieval_modifiers(&cli.retrieval)?;
    let root = cli.path.as_deref().unwrap_or(".");
    if let Some(res) = dispatch_primary_workflows(&cli, root) {
        return res;
    }
    let cli_gate = GateDispatchArgs {
        edit_check: cli.edit_check.as_deref(),
        affected: cli.affected.as_deref(),
        quality_baseline: cli.quality_baseline,
        quality_delta: cli.quality_delta || cli.safety.quality_ack.is_some(),
        scope: cli.safety.scope.as_deref(),
        quality_ack: cli
            .safety
            .quality_ack
            .as_ref()
            .map(|o| o.as_deref().unwrap_or("acknowledged")),
        ack_only: cli.safety.ack_only.as_deref(),
        show_stale: cli.safety.show_stale,
        allow_dirty: cli.safety.allow_dirty,
    };
    if let Some(res) = dispatch_gate_command(root, cli_gate) {
        return res;
    }

    if cli.clones {
        return crate::clones::execute_clones_command(root, cli.min_tokens);
    }
    if let Some(html_opt) = cli.html {
        return execute_html_export(root, html_opt.as_deref(), &cli.color_by);
    }
    if let Some(target) = cli.slice.as_deref() {
        return crate::slice::execute_slice_command_with_depth(
            root,
            Some(target),
            &cli.slice_flow,
            cli.slice_depth,
        );
    }
    let budget = cli.effective_budget_bytes();
    match &cli.command {
        Some(cmd) => dispatch_subcommand(cmd, &cli, root),
        None => dispatch_default_run(&cli, root, budget),
    }
}

/// Main CLI entry point.
pub fn run_cli() -> io::Result<()> {
    let args = std::env::args().map(|a| if a == "-v" { "-V".to_string() } else { a });
    let cli = Cli::parse_from(args);
    run_cli_args(cli)
}
