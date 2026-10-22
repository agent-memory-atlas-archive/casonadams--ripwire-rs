use super::args::*;
use super::queries::*;
use crate::commands::safety::{
    GateDispatchArgs, GitMetricsFlags, SafetyArgs, dispatch_gate_command,
    dispatch_git_metrics_with_flags,
};
use crate::domain::Interner;
use crate::graph::Graph;
use crate::query::map::*;
use crate::query::navigation::{NavContext, NavMode, execute_nav_symbol_query_in_root};
use crate::rank::PageRankResult;
use crate::workspace::{load_repo_analysis, load_repo_analysis_with_options};
use clap::{CommandFactory, FromArgMatches};
use std::io::{self, Write};
use std::path::Path;
pub fn execute_navigate_with<W: Write>(
    root: &str,
    args: &NavigateArgs,
    ctx: &NavContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    let sym_name = args.symbol.as_deref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "--symbol is required for navigate",
        )
    })?;
    let mode = if args.impact {
        NavMode::Impact
    } else if args.callers {
        NavMode::Callers
    } else if args.callees {
        NavMode::Callees
    } else {
        NavMode::FindSymbol
    };
    execute_nav_symbol_query_in_root(root, sym_name, mode, ctx, out)
}

pub fn execute_navigate(cli: &Cli, args: &NavigateArgs, default_root: &str) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(default_root);
    let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
    let ctx = NavContext {
        ingest: &ingest,
        graph: &graph,
        ranks: &pr.ranks,
        pr_iters: Some(pr.iterations),
        interner: &interner,
        page: crate::serialize::PageRequest {
            limit: cli.modifiers.limit,
            offset: cli.modifiers.offset,
        },
        max_depth: crate::commands::nav::impact_depth(cli),
    };
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, false);
    execute_navigate_with(root, args, &ctx, &mut stdout)?;
    stdout.finish()
}

pub fn execute_html_export(cli: &Cli, root: &str, out_file: Option<&str>) -> io::Result<()> {
    let color_by = cli.color_by.parse().unwrap_or_default();
    let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
    if let Some(path) = out_file {
        let mut file = std::fs::File::create(path)?;
        crate::serialize::html::write_call_graph_html(
            &mut file, &ingest, &graph, &pr.ranks, &interner, color_by, None,
        )
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
        )
    }
}

pub fn execute_stream_xml(root: &str) -> io::Result<()> {
    let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
    execute_stream_xml_with(root, &ingest, &graph, &pr, &interner, &mut stdout)?;
    stdout.finish()
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
    force_uncached: bool,
) -> io::Result<(crate::domain::IngestResult, Graph, PageRankResult, Interner)> {
    let cache_path = cli.modifiers.cache.as_deref().map(Path::new);
    let opts = crawler_options_for_cli(cli);
    load_repo_analysis_with_options(
        root,
        cache_path,
        cli.modifiers.no_cache || force_uncached,
        &opts,
        cli.scip.as_deref().map(Path::new),
    )
}

pub fn compute_effective_budget(
    pack_budget: Option<usize>,
    token_budget: Option<usize>,
    max_tokens: Option<usize>,
) -> Option<usize> {
    token_budget
        .or(max_tokens)
        .map(crate::query::map::token_budget_bytes)
        .or(pack_budget)
        .or(Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES))
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
    fn execute_verb<W: Write>(
        &self,
        ctx: &ExecutionContext<'_>,
        out: &mut crate::serialize::XmlWriter<'_, W>,
    ) -> io::Result<()>;

    fn force_plain_markdown(&self) -> bool {
        false
    }

    fn emits_plain_text(&self) -> bool {
        false
    }
}

pub fn run_cli_verb<V: CliVerb>(verb: &V, cli: &Cli, root: &str) -> Option<io::Result<()>> {
    if !verb.is_group_active() {
        return None;
    }
    Some((|| {
        let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
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
        let mut stdout = if verb.emits_plain_text() && !cli.is_json_out() {
            crate::serialize::XmlWriter::new(&mut stdout_lock, true)
        } else {
            cli.wrap_xml_writer(&mut stdout_lock, plain)
        };
        verb.execute_verb(&ctx, &mut stdout)?;
        stdout.finish()
    })())
}

pub fn split_csv_patterns(raw: &str) -> Vec<&str> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

fn execute_arch_rules_query<W: Write>(
    rules_file: &str,
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    stdout: &mut crate::serialize::XmlWriter<'_, W>,
) -> io::Result<()> {
    let content = std::fs::read_to_string(rules_file)?;
    let cfg = crate::gate::arch::ArchConfig::parse(&content);
    let violations = crate::gate::arch::check_architecture_rules(&cfg, ingest, graph, interner);
    crate::gate::arch::serialize_arch_xml(root, &violations, stdout)?;
    stdout.finish()?;
    if !violations.is_empty() {
        std::process::exit(2);
    }
    Ok(())
}

fn execute_arch_or_zoom_query<W: Write>(
    arch_args: &TopArchitectureArgs,
    ctx: &ExecutionContext<'_>,
    out: &mut crate::serialize::XmlWriter<'_, W>,
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
    fn execute_verb<W: Write>(
        &self,
        ctx: &ExecutionContext<'_>,
        out: &mut crate::serialize::XmlWriter<'_, W>,
    ) -> io::Result<()> {
        if let Some(res) = execute_arch_or_zoom_query(self, ctx, out) {
            return res;
        }
        if self.dead_code {
            execute_dead_code_query_paged(
                &super::arch::DeadCodeContext {
                    root: ctx.root,
                    sources: &crate::query::FileSources {
                        root: Path::new(ctx.root),
                    },
                    ingest: ctx.ingest,
                    graph: ctx.graph,
                    interner: ctx.interner,
                    page: ctx.cli,
                },
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
            let (ingest, _graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
            let mut stdout = io::stdout().lock();
            crate::metrics::run_retrieval_eval_for_repo(&ingest, &interner, &pr.ranks, &mut stdout)
        })());
    }
    if let Some(file) = cli.modifiers.batch.as_deref() {
        return Some((|| {
            let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
            let mut stdout_lock = io::stdout().lock();
            let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, false);
            execute_batch_file(root, file, &ingest, &graph, &pr, &interner, &mut stdout)?;
            stdout.finish()
        })());
    }
    crate::commands::run(cli, root)
        .or_else(|| run_cli_verb(&cli.arch, cli, root))
}

pub fn dispatch_default_run(cli: &Cli, root: &str, budget: Option<usize>) -> io::Result<()> {
    let emit_legend = !cli.is_legend_none();
    let legend_full = cli.legend.eq_ignore_ascii_case("full");
    let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, false);
    let rank_by = if cli.modifiers.stable || cli.modifiers.order.eq_ignore_ascii_case("stable") {
        "stable"
    } else {
        &cli.rank_by
    };
    let map_diff = crate::commands::safety::args(cli).map_diff;
    let changed_files = map_diff.then(|| crate::git::repo::working_tree_changes(root));
    let at = map_diff.then(|| {
        crate::git::history::compute_git_at_stamp(root).unwrap_or_else(|| "HEAD".to_string())
    });
    let mut args = MapRequest::new(root);
    args.rank_by = rank_by;
    args.pack_budget = budget;
    args.top_k = cli.top_k.or(args.top_k);
    args.emit_legend = emit_legend;
    args.legend_full = legend_full;
    args.map_diff = changed_files
        .as_ref()
        .zip(at.as_deref())
        .map(|(files, at)| MapDiff { files, at });
    args.no_inventory = cli.modifiers.no_inventory;
    execute_stream_xml_ranked(&args, &ingest, &graph, &pr, &interner, &mut stdout)?;
    stdout.finish()
}
fn execute_orient_subcommand(cli: &Cli, args: &OrientArgs, default_root: &str) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(default_root);
    let Some(task) = args.for_task.as_deref() else {
        return dispatch_default_run(cli, root, cli.effective_budget_bytes());
    };
    let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
    let sources = crate::query::FileSources {
        root: Path::new(root),
    };
    let at = crate::git::history::compute_git_at_stamp(root);
    let query = crate::query::commands::QueryContext {
        root,
        ingest: &ingest,
        graph: &graph,
        pr: &pr,
        interner: &interner,
        sources: &sources,
        commits: None,
        at: at.as_deref(),
    };
    let val = crate::commands::retrieval::orient_for_task(cli, &query, task);
    if let Some(error) = val["error"].as_str() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, error));
    }
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, false);
    crate::serialize::serialize_for_xml(&val, &mut stdout)?;
    stdout.finish()
}

fn execute_slice_for_cli(
    cli: &Cli,
    root: &str,
    target: Option<&str>,
    flow: &str,
    max_depth: u32,
) -> io::Result<()> {
    let target = target
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "slice target is required"))?;
    let flow = flow
        .parse()
        .map_err(|e: String| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let (ingest, _, _, interner) = load_repo_analysis_for_cli(cli, root, false)?;
    let registry = crate::ingest::LanguageRegistry::new().map_err(io::Error::other)?;
    let sources = crate::query::FileSources {
        root: Path::new(root),
    };
    let at = crate::git::history::compute_git_at_stamp(root);
    let report = crate::slice::query_slice(
        crate::slice::SliceRequest {
            root,
            target,
            flow,
            max_depth,
            at: at.as_deref(),
        },
        crate::slice::SliceContext {
            ingest: &ingest,
            interner: &interner,
            registry: &registry,
        },
        &sources,
    )
    .map_err(io::Error::other)?;
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, false);
    stdout.write_all(report.as_bytes())?;
    stdout.finish()
}

pub fn dispatch_subcommand(command: &Commands, cli: &Cli, default_root: &str) -> io::Result<()> {
    match command {
        Commands::Mcp(mcp_args) => crate::mcp::run_mcp_stdio_with_scip(
            mcp_args.path.as_deref().unwrap_or(default_root),
            cli.scip.as_deref().map(Path::new),
        ),
        Commands::Navigate(nav_args) => execute_navigate(cli, nav_args, default_root),
        Commands::Orient(orient_args) => execute_orient_subcommand(cli, orient_args, default_root),
        Commands::Gate(args) => execute_gate_subcommand(args, cli, default_root),
        Commands::Metrics(args) => execute_metrics_subcommand(args, default_root),
        Commands::Slice(slice_args) => execute_slice_for_cli(
            cli,
            default_root,
            slice_args.target.as_deref(),
            &slice_args.flow,
            cli.slice_depth,
        ),
        Commands::Skills(skills_args) => execute_skills_install(skills_args.path.as_deref()),
    }
}

fn execute_gate_subcommand(args: &GateArgs, cli: &Cli, default_root: &str) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(default_root);
    let safety = crate::commands::safety::args(cli);
    let inherited = gate_args_for_cli(cli, &safety);
    let gate = GateDispatchArgs {
        edit_check: args.edit_check.as_deref().or(inherited.edit_check),
        affected: args.affected.as_deref().or(inherited.affected),
        quality_baseline: args.quality_baseline || inherited.quality_baseline,
        quality_delta: args.quality_delta || inherited.quality_delta,
        ..inherited
    };
    if let Some(result) = dispatch_gate_command(root, gate, cli) {
        return result;
    }
    if args.clones {
        return crate::clones::execute_clones_command(root, args.min_tokens);
    }
    Ok(())
}

fn execute_metrics_subcommand(args: &MetricsArgs, default_root: &str) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(default_root);
    dispatch_git_metrics_with_flags(
        root,
        GitMetricsFlags {
            cochange: args.cochange.as_ref().map(|value| value.as_deref()),
            hotspots: args.hotspots,
            since: args.since.as_deref(),
            ..Default::default()
        },
    )
    .unwrap_or(Ok(()))
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
        return Some(crate::mcp::run_mcp_stdio_with_scip(
            root,
            cli.scip.as_deref().map(Path::new),
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
    let safety = crate::commands::safety::args(cli);
    if let Some(res) = dispatch_git_metrics_with_flags(
        root,
        GitMetricsFlags {
            cochange: cli.cochange.as_ref().map(|o| o.as_deref()),
            cochange_groups: safety.cochange_groups,
            cochange_recur: safety.cochange_recur,
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

fn gate_args_for_cli<'a>(cli: &'a Cli, safety: &'a SafetyArgs) -> GateDispatchArgs<'a> {
    GateDispatchArgs {
        edit_check: cli.edit_check.as_deref(),
        affected: cli.affected.as_deref(),
        quality_baseline: cli.quality_baseline,
        quality_delta: cli.quality_delta || safety.quality_ack.is_some(),
        scope: safety.scope.as_deref(),
        quality_ack: safety
            .quality_ack
            .as_ref()
            .map(|o| o.as_deref().unwrap_or("acknowledged")),
        ack_only: safety.ack_only.as_deref(),
        show_stale: safety.show_stale,
        allow_dirty: safety.allow_dirty,
        limit: cli.limit_opt(),
        offset: cli.offset_opt(),
    }
}

pub fn run_cli_args(cli: Cli) -> io::Result<()> {
    let refresh_clone = crate::commands::retrieval::refresh_clone_requested(&cli);
    let checkout = cli
        .path
        .as_deref()
        .filter(|p| crate::git::remote::is_remote_git_url(p))
        .map(|url| crate::git::remote::checkout_remote_root(url, refresh_clone))
        .transpose()?;
    validate_arch_modifiers(&cli.arch)?;
    crate::commands::retrieval::validate_modifiers(&cli)?;
    let root = checkout
        .as_deref()
        .and_then(std::path::Path::to_str)
        .or(cli.path.as_deref())
        .unwrap_or(".");
    if let Some(res) = dispatch_primary_workflows(&cli, root) {
        return res;
    }
    let safety = crate::commands::safety::args(&cli);
    if !matches!(cli.command.as_ref(), Some(Commands::Gate(_)))
        && let Some(res) = dispatch_gate_command(root, gate_args_for_cli(&cli, &safety), &cli)
    {
        return res;
    }

    if cli.clones {
        return crate::clones::execute_clones_command(root, cli.min_tokens);
    }
    if let Some(html_opt) = &cli.html {
        return execute_html_export(&cli, root, html_opt.as_deref());
    }
    if let Some(target) = cli.slice.as_deref() {
        return execute_slice_for_cli(&cli, root, Some(target), &cli.slice_flow, cli.slice_depth);
    }
    let budget = cli.effective_budget_bytes();
    match &cli.command {
        Some(cmd) => dispatch_subcommand(cmd, &cli, root),
        None => dispatch_default_run(&cli, root, budget),
    }
}

/// Parse the command line, including every command plugin's flags.
pub fn parse_from<I, T>(args: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let matches = crate::commands::augment(Cli::command()).try_get_matches_from(args)?;
    let mut cli = Cli::from_arg_matches(&matches)?;
    cli.plugins = PluginArgs::new(matches);
    Ok(cli)
}

/// Main CLI entry point.
pub fn run_cli() -> io::Result<()> {
    let args = std::env::args().map(|a| if a == "-v" { "-V".to_string() } else { a });
    let cli = parse_from(args).unwrap_or_else(|error| error.exit());
    run_cli_args(cli)
}
