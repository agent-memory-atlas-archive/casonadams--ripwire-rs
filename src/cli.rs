use crate::domain::Interner;
use crate::graph::Graph;
use crate::mcp::run_mcp_stdio;
use crate::navigate::{find_symbols_by_name, navigate_1hop, transitive_impact};
use crate::rank::PageRankResult;
use crate::serialize::{SerializeOptions, ToXmlString};
use clap::{Args, Parser, Subcommand};
use std::io;

/// CLI command line options.
#[derive(Parser, Debug, Default, PartialEq)]
#[command(
    name = "rw",
    version,
    about = "High-throughput agent context router and static analysis engine"
)]
pub struct Cli {
    /// Target repository path (default: current directory)
    #[arg(value_name = "PATH")]
    pub path: Option<String>,
    /// Fowler's Shotgun Surgery co-change mining
    #[arg(long)]
    pub cochange: Option<Option<String>>,

    /// Limit git history traversal to commits after this date or revision
    #[arg(long)]
    pub since: Option<String>,

    /// Show top maintenance hotspots (churn * ccx)
    #[arg(long)]
    pub hotspots: bool,

    /// Verify 1-hop caller compatibility against git HEAD
    #[arg(long)]
    pub edit_check: Option<String>,

    /// Show test files affected by changes
    #[arg(long)]
    pub affected: Option<String>,

    /// Pin quality baseline stamped with git HEAD
    #[arg(long)]
    pub quality_baseline: bool,

    /// Report code quality delta regressions vs baseline
    #[arg(long)]
    pub quality_delta: bool,

    /// Detect AST code duplication and clones (Type-1, Type-2, Type-3)
    #[arg(long)]
    pub clones: bool,

    /// Minimum token length for clone detection
    #[arg(long, default_value_t = 40)]
    pub min_tokens: usize,

    /// ARISE intra-procedural def-use variable slice (SYM[:VAR])
    #[arg(long)]
    pub slice: Option<String>,

    /// Dataflow direction for --slice (fwd, back, both)
    #[arg(long, default_value = "fwd")]
    pub slice_flow: String,

    /// Export interactive HTML call graph visualization to file
    #[arg(long, value_name = "FILE")]
    pub html: Option<Option<String>>,

    /// Node coloring mode for --html: lang, community, cx, churn, tested
    #[arg(long, default_value = "lang")]
    pub color_by: String,

    /// Ranking signal: pagerank, authority, hub
    #[arg(long, default_value = "pagerank")]
    pub rank_by: String,

    /// Streaming budget in bytes (default 65536)
    #[arg(long)]
    pub pack_budget: Option<usize>,

    /// Token budget (~4 bytes per token, alternative to --pack-budget)
    #[arg(long)]
    pub token_budget: Option<usize>,

    /// Top-level symbol navigation flags
    #[command(flatten)]
    pub nav: TopNavArgs,

    /// Top-level retrieval and architecture flags
    #[command(flatten)]
    pub retrieval: TopRetrievalArgs,

    /// Top-level workspace and audit flags
    #[command(flatten)]
    pub workspace: TopWorkspaceArgs,
    /// Workflow subcommand group
    #[command(subcommand)]
    pub command: Option<Commands>,
}
/// Top-level symbol navigation flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopNavArgs {
    /// Query 1-hop callers of a symbol
    #[arg(long, value_name = "SYM")]
    pub callers: Option<String>,

    /// Query 1-hop callees of a symbol
    #[arg(long, value_name = "SYM")]
    pub callees: Option<String>,

    /// Query transitive impact blast radius of a symbol
    #[arg(long, value_name = "SYM")]
    pub impact: Option<String>,

    /// Query all static use-sites of a symbol
    #[arg(long, value_name = "SYM")]
    pub uses: Option<String>,

    /// Query 1-hop callers and callees of a symbol
    #[arg(long = "find-symbol", value_name = "SYM")]
    pub find_symbol: Option<String>,
}

/// Top-level retrieval & architecture flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopRetrievalArgs {
    /// Task-lens relevance ranking
    #[arg(long = "for", value_name = "TASK")]
    pub task_for: Option<String>,

    /// Literal trigram text search
    #[arg(long, value_name = "PAT")]
    pub grep: Option<String>,

    /// Minimal connecting subgraph relating symbols (A,B,...)
    #[arg(long, value_name = "A,B")]
    pub connect: Option<String>,

    /// Shortest directed call path between two symbols (A:B)
    #[arg(long = "path-between", value_name = "A:B")]
    pub path_between: Option<String>,
}

/// Top-level workspace & audit flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopWorkspaceArgs {
    /// Find which branches or trees define a symbol
    #[arg(long, value_name = "SYM")]
    pub whereis: Option<String>,

    /// Authorship and bus-factor analysis
    #[arg(long)]
    pub owners: bool,

    /// Detect stale doc references
    #[arg(long = "doc-drift")]
    pub doc_drift: bool,

    /// Scan dark feature flags and environment reads
    #[arg(long)]
    pub flags: bool,

    /// Divergent branch content tracking
    #[arg(long = "stray-content")]
    pub stray_content: bool,

    /// Markdown docs referencing symbol in backticks
    #[arg(long, value_name = "SYM")]
    pub mentions: Option<String>,
}

/// Categorized workflow command groups.
#[derive(Subcommand, Debug, PartialEq)]
pub enum Commands {
    /// Exploration and task-based context orientation
    Orient(OrientArgs),
    /// Code navigation and call graph traversals (callers, callees, impact)
    Navigate(NavigateArgs),
    /// Pre-commit verification gates (edit-check, affected, quality-delta)
    Gate(GateArgs),
    /// Code complexity, churn, and architectural health metrics
    Metrics(MetricsArgs),
    /// ARISE intra-procedural def-use variable slicing
    Slice(SliceArgs),
    /// Launch stdio JSON-RPC Model Context Protocol server
    Mcp(McpArgs),
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct OrientArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

    /// Task description for relevance-ranked orientation
    #[arg(long = "for", value_name = "TASK")]
    pub for_task: Option<String>,
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct NavigateArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

    /// Target symbol name
    #[arg(short, long)]
    pub symbol: Option<String>,

    /// Show direct 1-hop callers
    #[arg(long)]
    pub callers: bool,

    /// Show direct 1-hop callees
    #[arg(long)]
    pub callees: bool,

    /// Show transitive blast radius
    #[arg(long)]
    pub impact: bool,
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct GateArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

    /// Verify 1-hop caller compatibility against git HEAD
    #[arg(long)]
    pub edit_check: Option<String>,

    /// Pin quality baseline stamped with git HEAD
    #[arg(long)]
    pub quality_baseline: bool,

    /// Measure quality delta against baseline
    #[arg(long)]
    pub quality_delta: bool,

    /// Show test files affected by changes
    #[arg(long)]
    pub affected: Option<String>,

    /// Detect AST code duplication and clones (Type-1, Type-2, Type-3)
    #[arg(long)]
    pub clones: bool,

    /// Minimum token length for clone detection
    #[arg(long, default_value_t = 40)]
    pub min_tokens: usize,
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct MetricsArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

    /// Show top maintenance hotspots (churn * ccx)
    #[arg(long)]
    pub hotspots: bool,

    /// Show file co-change coupling (Fowler's Shotgun Surgery)
    #[arg(long)]
    pub cochange: Option<Option<String>>,

    /// Limit git history traversal to commits after this date or revision
    #[arg(long)]
    pub since: Option<String>,
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct SliceArgs {
    /// Target variable criterion in symbol (e.g. sym:var)
    #[arg(value_name = "TARGET")]
    pub target: Option<String>,

    /// Dataflow direction (fwd, back, both)
    #[arg(long, default_value = "fwd")]
    pub flow: String,
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct McpArgs {
    /// Target workspace root to pin
    #[arg(value_name = "PATH")]
    pub path: Option<String>,
}

pub struct RankedStreamArgs<'a> {
    pub root: &'a str,
    pub rank_by: &'a str,
    pub pack_budget: Option<usize>,
}

pub fn execute_stream_xml_with<W: io::Write>(
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let args = RankedStreamArgs {
        root,
        rank_by: "pagerank",
        pack_budget: None,
    };
    execute_stream_xml_ranked(&args, ingest, graph, pr, interner, out)
}

pub fn execute_stream_xml_ranked<W: io::Write>(
    args: &RankedStreamArgs<'_>,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let budget_bytes = args
        .pack_budget
        .or(Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES));
    let rank_mode = args.rank_by.to_ascii_lowercase();

    let (ranks, rank_lbl) = if rank_mode == "authority" {
        let hits = graph.hits();
        (hits.authority, Some("authority".to_string()))
    } else if rank_mode == "hub" {
        let hits = graph.hits();
        (hits.hub, Some("hub".to_string()))
    } else {
        (pr.ranks.clone(), None)
    };

    let stream_ctx = crate::serialize::StreamXmlContext {
        ingest,
        graph,
        ranks: &ranks,
        pr_iters: pr.iterations,
        pr_converged: pr.converged,
        interner,
    };

    crate::serialize::serialize_xml_with_ranks(
        &stream_ctx,
        out,
        &SerializeOptions {
            root: args.root.to_string(),
            top_k: None,
            include_pr: true,
            pack_budget_bytes: budget_bytes,
            rank_by: rank_lbl,
        },
    )
}

fn load_repo_analysis(
    root: &str,
) -> io::Result<(
    crate::domain::IngestResult,
    Graph,
    crate::rank::PageRankResult,
    Interner,
)> {
    let path = std::path::Path::new(root);
    let files = crate::crawler::crawl_directory(path, &crate::crawler::CrawlerOptions::default());
    let registry =
        crate::ingest::LanguageRegistry::new().map_err(|e| io::Error::other(e.to_string()))?;
    let interner = Interner::new();
    let ingest = crate::ingest::ingest_crawled_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&crate::rank::PageRankConfig::default());
    Ok((ingest, graph, pr, interner))
}

fn compute_effective_budget(
    pack_budget: Option<usize>,
    token_budget: Option<usize>,
) -> Option<usize> {
    if let Some(tb) = token_budget {
        Some(tb.saturating_mul(4))
    } else {
        pack_budget.or(Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES))
    }
}

fn execute_stream_xml(root: &str) -> io::Result<()> {
    let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
    let mut stdout = io::stdout().lock();
    execute_stream_xml_with(root, &ingest, &graph, &pr, &interner, &mut stdout)
}
/// Context for symbol navigation queries.
pub struct NavContext<'a> {
    pub ingest: &'a crate::domain::IngestResult,
    pub graph: &'a Graph,
    pub ranks: &'a [f64],
    pub interner: &'a Interner,
}

pub fn print_symbol_nav_to<W: io::Write>(
    id: crate::domain::NodeId,
    args: &NavigateArgs,
    sym_name: &str,
    ctx: &NavContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    if args.impact {
        let impact = transitive_impact(id, ctx.ingest, ctx.graph, ctx.ranks, ctx.interner, None);
        return crate::navigate::serialize_impact_xml(sym_name, &impact, out);
    }
    let Some(nav) = navigate_1hop(id, ctx.ingest, ctx.graph, ctx.ranks, ctx.interner) else {
        return Ok(());
    };
    if args.callers {
        crate::navigate::serialize_callers_xml(sym_name, &nav.callers, out)
    } else if args.callees {
        crate::navigate::serialize_callees_xml(sym_name, &nav.callees, out)
    } else {
        crate::navigate::serialize_find_symbol_xml(&nav, out)
    }
}

pub fn execute_navigate_with<W: io::Write>(
    args: &NavigateArgs,
    ctx: &NavContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    let sym_name = match args.symbol.as_deref() {
        Some(s) => s,
        None => {
            writeln!(out, "Error: --symbol is required for navigate commands")?;
            return Ok(());
        }
    };

    let ids = find_symbols_by_name(sym_name, ctx.ingest, ctx.interner);
    if ids.is_empty() {
        writeln!(out, "Symbol '{}' not found", sym_name)?;
        return Ok(());
    }

    for id in ids {
        print_symbol_nav_to(id, args, sym_name, ctx, out)?;
    }
    Ok(())
}

fn execute_navigate(args: &NavigateArgs) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(".");
    let state = crate::mcp::McpState::load(root)
        .ok_or_else(|| io::Error::other("failed to load repository"))?;
    let ctx = NavContext {
        ingest: &state.ingest,
        graph: &state.graph,
        ranks: &state.ranks,
        interner: &state.interner,
    };

    let mut stdout = io::stdout().lock();
    execute_navigate_with(args, &ctx, &mut stdout)
}

fn print_cochange_partners<W: io::Write>(
    file: &str,
    partners: &[crate::git::cochange::CoPartner],
    out: &mut W,
) -> io::Result<()> {
    writeln!(out, "<f path=\"{}\" partners=\"{}\">", file, partners.len())?;
    for p in partners {
        let attr = if !p.dep_capable {
            " dep_capable=\"0\""
        } else if p.surprising {
            " surprising=\"1\""
        } else {
            ""
        };
        writeln!(
            out,
            "  <partner path=\"{}\" together=\"{}\" deg=\"{:.2}\" deg_rev=\"{:.2}\" recur=\"{}\"{} />",
            p.file_path, p.together, p.deg, p.deg_rev, p.recur, attr
        )?;
    }
    writeln!(out, "</f>")
}

fn print_cochange_pairs<W: io::Write>(
    pairs: &[crate::git::cochange::CoPair],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<cochange schema=\"ripwire.cochange/v1\" pairs=\"{}\">",
        pairs.len()
    )?;
    for p in pairs {
        let attr = if !p.dep_capable {
            " dep_capable=\"0\""
        } else if p.surprising {
            " surprising=\"1\""
        } else {
            ""
        };
        writeln!(
            out,
            "  <pair a=\"{}\" b=\"{}\" together=\"{}\" deg=\"{:.2}\" recur=\"{}\"{} />",
            p.file_a, p.file_b, p.together, p.deg, p.recur, attr
        )?;
    }
    writeln!(out, "</cochange>")
}

pub fn execute_cochange_with<W: io::Write>(
    probed_file: Option<&str>,
    since_str: Option<&str>,
    commits: &[crate::git::history::CommitInfo],
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    let anchor_epoch = commits.first().map(|c| c.timestamp).unwrap_or(0);
    let scope = crate::git::history::parse_since_scope(since_str, anchor_epoch);
    let filtered = crate::git::history::filter_commits_by_scope(commits, &scope);
    let coupling = crate::git::cochange::StaticCouplingMap::build(
        &state.ingest,
        &state.interner,
        Some(&state.graph),
    );
    let opts = crate::git::cochange::CochangeOptions::default();

    if let Some(file) = probed_file {
        let partners = crate::git::cochange::mine_cochange_for_file(
            file,
            filtered,
            Some(&coupling),
            Some(&state.ingest),
            Some(&state.interner),
            &opts,
        );
        print_cochange_partners(file, &partners, out)
    } else {
        let pairs = crate::git::cochange::mine_repo_cochange_pairs(
            filtered,
            Some(&coupling),
            Some(&state.ingest),
            Some(&state.interner),
            &opts,
        );
        print_cochange_pairs(&pairs, out)
    }
}

fn execute_with_git_state<F>(root: &str, runner: F) -> io::Result<()>
where
    F: FnOnce(
        &[crate::git::history::CommitInfo],
        &crate::mcp::McpState,
        &mut io::StdoutLock<'_>,
    ) -> io::Result<()>,
{
    let state = crate::mcp::McpState::load(root)
        .ok_or_else(|| io::Error::other("failed to load repository"))?;
    let empty_commits = Vec::new();
    let commits = state.commits.as_ref().unwrap_or(&empty_commits);
    let mut stdout = io::stdout().lock();
    runner(commits, &state, &mut stdout)
}

fn dispatch_git_metrics_command(
    root: &str,
    cochange: Option<Option<&str>>,
    hotspots: bool,
    since: Option<&str>,
) -> Option<io::Result<()>> {
    if let Some(probed) = cochange {
        return Some(execute_with_git_state(root, |commits, state, out| {
            execute_cochange_with(probed, since, commits, state, out)
        }));
    }
    if hotspots {
        return Some(execute_with_git_state(root, |commits, state, out| {
            execute_hotspots_with(since, commits, state, out)
        }));
    }
    None
}

pub fn execute_hotspots_with<W: io::Write>(
    since_str: Option<&str>,
    commits: &[crate::git::history::CommitInfo],
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    let anchor_epoch = commits.first().map(|c| c.timestamp).unwrap_or(0);
    let scope = crate::git::history::parse_since_scope(since_str, anchor_epoch);
    let filtered = crate::git::history::filter_commits_by_scope(commits, &scope);
    let churn_map = crate::git::history::compute_file_churn(
        filtered,
        anchor_epoch,
        crate::git::history::MERGE_BOMB_MAX_FILES,
    );
    let counts: std::collections::HashMap<String, u32> = churn_map
        .into_iter()
        .map(|(path, fc)| (path, fc.commits))
        .collect();
    let report = crate::metrics::compute_hotspots(&state.ingest, &state.interner, &counts);
    report.write_xml(out)
}

pub fn execute_edit_check_with<W: io::Write>(
    sym_name: &str,
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    let ids = crate::navigate::find_symbols_by_name(sym_name, &state.ingest, &state.interner);
    let Some(&sym_id) = ids.first() else {
        writeln!(out, "Symbol '{}' not found", sym_name)?;
        return Ok(());
    };

    let sym = &state.ingest.symbols[sym_id as usize];
    let rel_path = state
        .interner
        .resolve(&state.ingest.files[sym.file_id as usize].path);
    let head_contract = crate::gate::contract::load_head_symbol_contract(
        std::path::Path::new(&state.root),
        rel_path,
        sym_name,
        sym.lang,
    )
    .ok()
    .flatten();

    if let Some(report) = crate::gate::edit_check::check_symbol_contract(
        sym_id,
        head_contract.as_ref(),
        &state.ingest,
        &state.graph,
        &state.interner,
    ) {
        report.write_xml(out)?;
    }
    Ok(())
}

pub fn execute_affected_with<W: io::Write>(
    specs_str: &str,
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    let specs: Vec<&str> = specs_str.split(',').map(|s| s.trim()).collect();
    let report = crate::gate::affected::find_affected_tests(
        &specs,
        &state.ingest,
        &state.graph,
        &state.interner,
    );
    report.write_xml(out)
}
pub fn execute_quality_baseline_with<W: io::Write>(
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    let head_sha = state
        .commits
        .as_ref()
        .and_then(|c| c.first())
        .map(|c| c.id.as_str())
        .unwrap_or("HEAD");
    let snap = crate::gate::baseline::compute_snapshot(
        &state.ingest,
        &state.graph,
        &state.interner,
        head_sha,
    );
    let baseline_path =
        std::path::Path::new(&state.root).join(crate::gate::baseline::DEFAULT_BASELINE_FILE);
    crate::gate::baseline::write_baseline_file(&snap, &baseline_path)?;
    writeln!(
        out,
        "rwire: pinned quality baseline stamped at {head_sha} -> {}",
        baseline_path.display()
    )
}

pub fn execute_quality_delta_with<W: io::Write>(
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<bool> {
    let report = crate::gate::quality_delta::evaluate_quality_delta(&state.root, state)?;
    report.write_xml(out)?;
    Ok(report.has_gating_failures())
}

#[derive(Default)]
struct GateDispatchArgs<'a> {
    edit_check: Option<&'a str>,
    affected: Option<&'a str>,
    quality_baseline: bool,
    quality_delta: bool,
}

impl GateDispatchArgs<'_> {
    fn is_active(&self) -> bool {
        self.edit_check.is_some()
            || self.affected.is_some()
            || self.quality_baseline
            || self.quality_delta
    }
}

fn run_quality_delta_gate<W: io::Write>(
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    match execute_quality_delta_with(state, out) {
        Ok(has_failures) if has_failures => Err(io::Error::other("quality regressions detected")),
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

fn execute_gate_action<W: io::Write>(
    args: GateDispatchArgs<'_>,
    state: &crate::mcp::McpState,
    out: &mut W,
) -> Option<io::Result<()>> {
    if let Some(sym) = args.edit_check {
        return Some(execute_edit_check_with(sym, state, out));
    }
    if let Some(spec) = args.affected {
        return Some(execute_affected_with(spec, state, out));
    }
    if args.quality_baseline {
        return Some(execute_quality_baseline_with(state, out));
    }
    if args.quality_delta {
        return Some(run_quality_delta_gate(state, out));
    }
    None
}

fn dispatch_gate_command(root: &str, args: GateDispatchArgs<'_>) -> Option<io::Result<()>> {
    if !args.is_active() {
        return None;
    }
    let state = match crate::mcp::McpState::load(root) {
        Some(s) => s,
        None => return Some(Err(io::Error::other("failed to load repository"))),
    };
    let mut stdout = io::stdout().lock();
    execute_gate_action(args, &state, &mut stdout)
}

fn execute_nav_symbol_query<W: io::Write>(
    sym: &str,
    nav_mode: u8,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let ids = crate::navigate::find_symbols_by_name(sym, ingest, interner);
    let Some(&id) = ids.first() else {
        let flag_name = match nav_mode {
            0 => "--callers",
            1 => "--callees",
            2 => "--impact",
            3 => "--uses",
            _ => "--find-symbol",
        };
        return Err(io::Error::other(format!(
            "rwire: {flag_name} symbol not found: {sym}"
        )));
    };
    if nav_mode == 3 {
        return crate::navigate::serialize_uses_xml(sym, ingest, interner, out);
    }
    match nav_mode {
        0 => {
            let Some(nav) = crate::navigate::navigate_1hop(id, ingest, graph, ranks, interner)
            else {
                return Ok(());
            };
            crate::navigate::serialize_callers_xml(sym, &nav.callers, out)
        }
        1 => {
            let Some(nav) = crate::navigate::navigate_1hop(id, ingest, graph, ranks, interner)
            else {
                return Ok(());
            };
            crate::navigate::serialize_callees_xml(sym, &nav.callees, out)
        }
        2 => {
            let impact =
                crate::navigate::transitive_impact(id, ingest, graph, ranks, interner, None);
            crate::navigate::serialize_impact_xml(sym, &impact, out)
        }
        4 => {
            let Some(nav) = crate::navigate::navigate_1hop(id, ingest, graph, ranks, interner)
            else {
                return Ok(());
            };
            crate::navigate::serialize_find_symbol_xml(&nav, out)
        }
        _ => Ok(()),
    }
}

fn dispatch_top_nav(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let (sym, nav_mode) = if let Some(s) = cli.nav.callers.as_deref() {
        (s, 0)
    } else if let Some(s) = cli.nav.callees.as_deref() {
        (s, 1)
    } else if let Some(s) = cli.nav.impact.as_deref() {
        (s, 2)
    } else if let Some(s) = cli.nav.uses.as_deref() {
        (s, 3)
    } else {
        let s = cli.nav.find_symbol.as_deref()?;
        (s, 4)
    };

    Some((|| {
        let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
        let mut stdout = io::stdout().lock();
        execute_nav_symbol_query(
            sym,
            nav_mode,
            &ingest,
            &graph,
            &pr.ranks,
            &interner,
            &mut stdout,
        )
    })())
}

fn dispatch_top_retrieval(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let ret = &cli.retrieval;
    if ret.task_for.is_none()
        && ret.grep.is_none()
        && ret.connect.is_none()
        && ret.path_between.is_none()
    {
        return None;
    }
    Some((|| {
        let state = crate::mcp::McpState::load(root)
            .ok_or_else(|| io::Error::other("failed to load repository"))?;
        let mut stdout = io::stdout().lock();
        if let Some(task) = ret.task_for.as_deref() {
            let val = crate::mcp::retrieval::call_for_task(
                &state.ingest,
                &state.interner,
                &state.pr.ranks,
                task,
                None,
            );
            crate::serialize::serialize_for_xml(&val, &mut stdout)?;
        } else if let Some(pat) = ret.grep.as_deref() {
            let val = crate::mcp::retrieval::call_grep(&state.ingest, &state.interner, pat, None);
            crate::serialize::serialize_grep_xml(&val, &mut stdout)?;
        } else if let Some(syms) = ret.connect.as_deref() {
            let sym_list: Vec<String> = syms
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let val = crate::mcp::graph_tools::call_connect(
                &state.ingest,
                &state.graph,
                &state.interner,
                &sym_list,
            );
            crate::serialize::serialize_connect_xml(&val, &mut stdout)?;
        } else if let Some(pair) = ret.path_between.as_deref() {
            let parts: Vec<&str> = pair.split(':').collect();
            let from = parts.first().copied().unwrap_or("");
            let to = parts.get(1).copied().unwrap_or("");
            let val = crate::mcp::graph_tools::call_path_between(
                &state.ingest,
                &state.graph,
                &state.interner,
                from,
                to,
            );
            crate::serialize::serialize_path_between_xml(&val, &mut stdout)?;
        }
        Ok(())
    })())
}

fn dispatch_top_workspace(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let ws = &cli.workspace;
    if ws.whereis.is_none()
        && !ws.owners
        && !ws.doc_drift
        && !ws.flags
        && !ws.stray_content
        && ws.mentions.is_none()
    {
        return None;
    }
    Some((|| {
        let state = crate::mcp::McpState::load(root)
            .ok_or_else(|| io::Error::other("failed to load repository"))?;
        let mut stdout = io::stdout().lock();
        if let Some(sym) = ws.whereis.as_deref() {
            let val = crate::mcp::doc_tools::call_whereis(&state.ingest, &state.interner, sym);
            crate::serialize::serialize_whereis_xml(&val, &mut stdout)?;
        } else if ws.owners {
            let val = crate::mcp::doc_tools::call_owners(state.commits.as_deref(), None);
            crate::serialize::serialize_owners_xml(&val, &mut stdout)?;
        } else if ws.doc_drift {
            let val = crate::mcp::doc_tools::call_doc_drift(&state.ingest, &state.interner);
            crate::serialize::serialize_doc_drift_xml(&val, &mut stdout)?;
        } else if ws.flags {
            let val = crate::mcp::doc_tools::call_flags(&state.ingest, &state.interner);
            crate::serialize::serialize_flags_xml(&val, &mut stdout)?;
        } else if ws.stray_content {
            let val = crate::mcp::doc_tools::call_stray_content(state.commits.as_deref());
            crate::serialize::serialize_stray_content_xml(&val, &mut stdout)?;
        } else if let Some(sym) = ws.mentions.as_deref() {
            let val = crate::mcp::doc_tools::call_mentions(&state.ingest, &state.interner, sym);
            crate::serialize::serialize_mentions_xml(&val, &mut stdout)?;
        }
        Ok(())
    })())
}

/// Execute CLI from parsed options.
pub fn run_cli_args(cli: Cli) -> io::Result<()> {
    let root = cli.path.as_deref().unwrap_or(".");
    if let Some(res) = dispatch_top_nav(&cli, root) {
        return res;
    }
    if let Some(res) = dispatch_top_retrieval(&cli, root) {
        return res;
    }
    if let Some(res) = dispatch_top_workspace(&cli, root) {
        return res;
    }
    if let Some(res) = dispatch_git_metrics_command(
        root,
        cli.cochange.as_ref().map(|o| o.as_deref()),
        cli.hotspots,
        cli.since.as_deref(),
    ) {
        return res;
    }

    let cli_gate = GateDispatchArgs {
        edit_check: cli.edit_check.as_deref(),
        affected: cli.affected.as_deref(),
        quality_baseline: cli.quality_baseline,
        quality_delta: cli.quality_delta,
    };
    if let Some(res) = dispatch_gate_command(root, cli_gate) {
        return res;
    }

    if cli.clones {
        return crate::clones::execute_clones_command(root, cli.min_tokens);
    }
    if let Some(Commands::Gate(gate_args)) = &cli.command
        && gate_args.clones
    {
        let gate_root = gate_args.path.as_deref().unwrap_or(root);
        return crate::clones::execute_clones_command(gate_root, gate_args.min_tokens);
    }

    if let Some(html_opt) = cli.html {
        let out_file = html_opt.unwrap_or_else(|| "rwire.html".to_string());
        return execute_html_export(root, &out_file, &cli.color_by);
    }

    if let Some(Commands::Slice(slice_args)) = &cli.command {
        return crate::slice::execute_slice_command(
            root,
            slice_args.target.as_deref(),
            &slice_args.flow,
        );
    }

    if let Some(target) = cli.slice.as_deref() {
        return crate::slice::execute_slice_command(root, Some(target), &cli.slice_flow);
    }

    let budget = compute_effective_budget(cli.pack_budget, cli.token_budget);
    match &cli.command {
        Some(cmd) => dispatch_subcommand(cmd),
        None => dispatch_default_run(&cli, root, budget),
    }
}

fn dispatch_default_run(cli: &Cli, root: &str, budget: Option<usize>) -> io::Result<()> {
    if cli.rank_by == "pagerank" && budget == Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES) {
        execute_stream_xml(root)
    } else {
        let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
        let mut stdout = io::stdout().lock();
        let args = RankedStreamArgs {
            root,
            rank_by: &cli.rank_by,
            pack_budget: budget,
        };
        execute_stream_xml_ranked(&args, &ingest, &graph, &pr, &interner, &mut stdout)
    }
}

fn dispatch_subcommand(command: &Commands) -> io::Result<()> {
    match command {
        Commands::Mcp(mcp_args) => run_mcp_stdio(mcp_args.path.as_deref().unwrap_or(".")),
        Commands::Navigate(nav_args) => execute_navigate(nav_args),
        Commands::Orient(orient_args) => {
            execute_stream_xml(orient_args.path.as_deref().unwrap_or("."))
        }
        Commands::Gate(gate_args) => {
            let root = gate_args.path.as_deref().unwrap_or(".");
            let sub_gate = GateDispatchArgs {
                edit_check: gate_args.edit_check.as_deref(),
                affected: gate_args.affected.as_deref(),
                quality_baseline: gate_args.quality_baseline,
                quality_delta: gate_args.quality_delta,
            };
            if let Some(res) = dispatch_gate_command(root, sub_gate) {
                return res;
            }
            println!("rwire gate: pre-commit verification gates (Wave 2 feature)");
            Ok(())
        }
        Commands::Metrics(metrics_args) => {
            let root = metrics_args.path.as_deref().unwrap_or(".");
            if let Some(res) = dispatch_git_metrics_command(
                root,
                metrics_args.cochange.as_ref().map(|o| o.as_deref()),
                metrics_args.hotspots,
                metrics_args.since.as_deref(),
            ) {
                return res;
            }
            println!("rwire metrics: code complexity and health metrics (Wave 2 feature)");
            Ok(())
        }
        Commands::Slice(_) => {
            println!("rwire slice: ARISE variable dataflow slicing (Wave 3 feature)");
            Ok(())
        }
    }
}

fn execute_html_export(root: &str, out_file: &str, color_by_str: &str) -> io::Result<()> {
    let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
    let color_by = crate::serialize::html::ColorBy::from_str_opt(color_by_str).unwrap_or_default();
    let mut file = std::fs::File::create(out_file)?;
    crate::serialize::html::write_call_graph_html(
        &mut file, &ingest, &graph, &pr.ranks, &interner, color_by, None,
    )?;
    println!("rwire: wrote interactive HTML call graph to {out_file}");
    Ok(())
}

/// Main CLI entry point.
pub fn run_cli() -> io::Result<()> {
    let cli = Cli::parse();
    run_cli_args(cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let help = Cli::try_parse_from(["rw", "--help"]);
        assert!(help.is_err()); // --help exits with help message

        let default_run = Cli::try_parse_from(["rw", "."]).unwrap();
        assert_eq!(default_run.path, Some(".".into()));
        assert!(default_run.command.is_none());

        let mcp_cmd = Cli::try_parse_from(["rw", "mcp"]).unwrap();
        assert_eq!(mcp_cmd.command, Some(Commands::Mcp(McpArgs::default())));

        let nav_cmd =
            Cli::try_parse_from(["rw", "navigate", "--symbol", "main", "--callers"]).unwrap();
        assert!(matches!(nav_cmd.command, Some(Commands::Navigate(_))));

        let orient_cmd = Cli::try_parse_from(["rw", "orient", "--for", "auth"]).unwrap();
        assert!(matches!(orient_cmd.command, Some(Commands::Orient(_))));
    }
    #[test]
    fn test_gate_dispatch_args_is_active() {
        let empty = GateDispatchArgs::default();
        assert!(!empty.is_active());
        assert!(dispatch_gate_command(".", empty).is_none());

        let with_ec = GateDispatchArgs {
            edit_check: Some("main"),
            ..Default::default()
        };
        assert!(with_ec.is_active());

        let with_aff = GateDispatchArgs {
            affected: Some("src/main.rs"),
            ..Default::default()
        };
        assert!(with_aff.is_active());

        let with_qb = GateDispatchArgs {
            quality_baseline: true,
            ..Default::default()
        };
        assert!(with_qb.is_active());

        let with_qd = GateDispatchArgs {
            quality_delta: true,
            ..Default::default()
        };
        assert!(with_qd.is_active());
    }

    #[test]
    fn test_compute_effective_budget() {
        assert_eq!(compute_effective_budget(Some(1024), None), Some(1024));
        assert_eq!(compute_effective_budget(None, Some(100)), Some(400));
        assert_eq!(
            compute_effective_budget(None, None),
            Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES)
        );
    }
}
