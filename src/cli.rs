use crate::domain::Interner;
use crate::graph::Graph;
use crate::mcp::run_mcp_stdio;
use crate::navigate::{find_symbols_by_name, navigate_1hop, transitive_impact};
use crate::rank::PageRankResult;
use crate::serialize::{SerializeOptions, ToXmlString};
use clap::{Args, Parser, Subcommand};
use std::io::{self, Write};

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
    #[arg(
        long = "pack-budget",
        visible_alias = "pack-budget-bytes",
        value_name = "BYTES"
    )]
    pub pack_budget: Option<usize>,
    /// Token budget (~4 bytes per token, alternative to --pack-budget)
    #[arg(long)]
    pub token_budget: Option<usize>,

    /// Budget the map to ~N tokens (~4 bytes per token, alias for --token-budget)
    #[arg(long = "max-tokens", value_name = "N")]
    pub max_tokens: Option<usize>,

    /// Keep only the N highest-ranked symbols (e.g. 200)
    #[arg(long = "top-k", value_name = "N")]
    pub top_k: Option<usize>,

    /// Schema legend posture: compact (default, in-context schema comment) or none
    #[arg(long, default_value = "compact")]
    pub legend: String,
    /// Top-level symbol navigation flags
    #[command(flatten)]
    pub nav: TopNavArgs,

    /// Top-level retrieval and architecture flags
    #[command(flatten)]
    pub retrieval: TopRetrievalArgs,

    /// Top-level workspace and audit flags
    #[command(flatten)]
    pub workspace: TopWorkspaceArgs,

    /// Top-level detail ladder flags
    #[command(flatten)]
    pub detail: TopDetailArgs,

    /// Top-level pre-PR safety flags
    #[command(flatten)]
    pub safety: TopSafetyArgs,

    /// Top-level architecture and discovery flags
    #[command(flatten)]
    pub arch: TopArchitectureArgs,

    /// Top-level global modifiers and batching flags
    #[command(flatten)]
    pub modifiers: TopModifierArgs,
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
/// Top-level detail ladder flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopDetailArgs {
    /// Expand full symbol bodies and callee signatures (SYM,SYM...)
    #[arg(long, value_name = "SYMS")]
    pub expand: Option<String>,

    /// Outline control-flow skeletons of symbols (SYM,SYM...)
    #[arg(long, value_name = "SYMS")]
    pub outline: Option<String>,

    /// Pack bodies of the top N highest-ranked symbols
    #[arg(long = "pack-top-n", value_name = "N")]
    pub pack_top_n: Option<usize>,

    /// Orient file by file showing each file's top symbols
    #[arg(long)]
    pub tree: bool,
}
/// Top-level pre-PR safety and verification flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopSafetyArgs {
    /// Before a PR: name tests to run and untested blast radius (exit 4 if non-empty)
    #[arg(long = "test-gate", value_name = "FILES")]
    pub test_gate: Option<Option<String>>,

    /// Situational awareness for a change: blast radius + tests + co-change (default: git diff)
    #[arg(long, value_name = "FILES")]
    pub situ: Option<Option<String>>,

    /// Safe delete assessment joining callers, blast radius, tests, and history
    #[arg(long = "safe-delete", value_name = "SYM")]
    pub safe_delete: Option<String>,

    /// Show what a test file actually covers: non-test symbols it reaches
    #[arg(long, value_name = "TESTFILE")]
    pub exercises: Option<String>,
}
/// Top-level architecture and discovery flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopArchitectureArgs {
    /// Internal functions with zero incoming callers across the repository
    #[arg(long = "dead-code")]
    pub dead_code: bool,

    /// Cluster call graph into cohesive modules (Louvain community detection)
    #[arg(long)]
    pub communities: bool,

    /// Inspect members and bridges of one specific community module
    #[arg(long, value_name = "ID")]
    pub community: Option<u32>,

    /// File-to-file dependency graph with instability and cycle metrics
    #[arg(long)]
    pub deps: bool,

    /// Ego-graph around a symbol
    #[arg(long, value_name = "SYM")]
    pub around: Option<String>,

    /// Depth for --around ego-graph (default: 1)
    #[arg(long = "around-depth", default_value_t = 1)]
    pub around_depth: u32,

    /// Fanout limit for --around ego-graph (default: 32)
    #[arg(long = "around-fanout", default_value_t = 32)]
    pub around_fanout: usize,

    /// Names used but never defined in the index (external stdlib/deps surface)
    #[arg(long = "external-surface")]
    pub external_surface: bool,
}
/// Top-level global modifiers and batching flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopModifierArgs {
    /// Emit JSON instead of XML
    #[arg(long)]
    pub json: bool,

    /// Paginate: maximum number of items to emit
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,

    /// Paginate: number of items to skip
    #[arg(long, value_name = "M")]
    pub offset: Option<usize>,

    /// Emit ordering: important-first (default), important-last, or stable
    #[arg(long, default_value = "important-first")]
    pub order: String,

    /// Run several lookups in one turn from newline-delimited verb:arg file
    #[arg(long, value_name = "FILE")]
    pub batch: Option<String>,
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

    /// Shortest directed call path between two symbols (A:B or A,B)
    #[arg(long = "path-between", visible_alias = "path", value_name = "A:B")]
    pub path_between: Option<String>,

    /// Retrieve best-in-class existing instance of code to imitate
    #[arg(long, value_name = "TASK|KIND")]
    pub exemplar: Option<String>,

    /// Show one interface and every type implementing it
    #[arg(long, value_name = "TYPE")]
    pub lego: Option<String>,

    /// Search plans, designs, and docs for relevant notes
    #[arg(long, value_name = "TASK")]
    pub recall: Option<String>,

    /// Map a stack trace, sanitizer report or compiler error onto indexed symbols
    #[arg(long = "from-trace", value_name = "FILE")]
    pub from_trace: Option<String>,
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
    pub top_k: Option<usize>,
    pub emit_legend: bool,
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
        top_k: None,
        emit_legend: true,
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
            top_k: args.top_k,
            include_pr: true,
            pack_budget_bytes: budget_bytes,
            rank_by: rank_lbl,
            emit_legend: args.emit_legend,
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
    max_tokens: Option<usize>,
) -> Option<usize> {
    if let Some(tb) = token_budget.or(max_tokens) {
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
        "ripwire: pinned quality baseline stamped at {head_sha} -> {}",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavMode {
    Callers,
    Callees,
    Impact,
    Uses,
    FindSymbol,
}

fn serialize_1hop_mode<W: io::Write>(
    sym: &str,
    mode: NavMode,
    nav: &crate::navigate::NavNeighborhood,
    out: &mut W,
) -> io::Result<()> {
    match mode {
        NavMode::Callers => crate::navigate::serialize_callers_xml(sym, &nav.callers, out),
        NavMode::Callees => crate::navigate::serialize_callees_xml(sym, &nav.callees, out),
        NavMode::FindSymbol => crate::navigate::serialize_find_symbol_xml(nav, out),
        _ => Ok(()),
    }
}

pub fn execute_nav_symbol_query<W: io::Write>(
    sym: &str,
    mode: NavMode,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let ids = crate::navigate::find_symbols_by_name(sym, ingest, interner);
    let Some(&id) = ids.first() else {
        let flag = match mode {
            NavMode::Callers => "--callers",
            NavMode::Callees => "--callees",
            NavMode::Impact => "--impact",
            NavMode::Uses => "--uses",
            NavMode::FindSymbol => "--find-symbol",
        };
        return Err(io::Error::other(format!(
            "ripwire: {flag} symbol not found: {sym}"
        )));
    };
    match mode {
        NavMode::Uses => crate::navigate::serialize_uses_xml(sym, ingest, interner, out),
        NavMode::Impact => {
            let impact =
                crate::navigate::transitive_impact(id, ingest, graph, ranks, interner, None);
            crate::navigate::serialize_impact_xml(sym, &impact, out)
        }
        NavMode::Callers | NavMode::Callees | NavMode::FindSymbol => {
            if let Some(nav) = crate::navigate::navigate_1hop(id, ingest, graph, ranks, interner) {
                serialize_1hop_mode(sym, mode, &nav, out)?;
            }
            Ok(())
        }
    }
}

fn dispatch_top_nav(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let (sym, mode) = if let Some(s) = cli.nav.callers.as_deref() {
        (s, NavMode::Callers)
    } else if let Some(s) = cli.nav.callees.as_deref() {
        (s, NavMode::Callees)
    } else if let Some(s) = cli.nav.impact.as_deref() {
        (s, NavMode::Impact)
    } else if let Some(s) = cli.nav.uses.as_deref() {
        (s, NavMode::Uses)
    } else {
        let s = cli.nav.find_symbol.as_deref()?;
        (s, NavMode::FindSymbol)
    };

    Some((|| {
        let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
        let mut stdout = io::stdout().lock();
        if cli.modifiers.json {
            let ids = crate::navigate::find_symbols_by_name(sym, &ingest, &interner);
            if let Some(&id) = ids.first() {
                if mode == NavMode::Impact {
                    let impact = crate::navigate::transitive_impact(
                        id, &ingest, &graph, &pr.ranks, &interner, None,
                    );
                    serde_json::to_writer_pretty(&mut stdout, &impact)?;
                    writeln!(stdout)?;
                    return Ok(());
                } else if let Some(nav) =
                    crate::navigate::navigate_1hop(id, &ingest, &graph, &pr.ranks, &interner)
                {
                    serde_json::to_writer_pretty(&mut stdout, &nav)?;
                    writeln!(stdout)?;
                    return Ok(());
                }
            }
        }
        execute_nav_symbol_query(
            sym,
            mode,
            &ingest,
            &graph,
            &pr.ranks,
            &interner,
            &mut stdout,
        )
    })())
}

pub fn execute_connect_query<W: io::Write>(
    syms: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let sym_list: Vec<String> = syms
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    for s in &sym_list {
        if crate::navigate::find_symbols_by_name(s, ingest, interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire: --connect symbol not found: {s}"
            )));
        }
    }
    let val = crate::mcp::graph_tools::call_connect(ingest, graph, interner, &sym_list);
    crate::serialize::serialize_connect_xml(&val, out)
}

fn split_endpoint_pair(pair: &str) -> (&str, &str) {
    if let Some((from, to)) = pair.split_once(',') {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.split_once("->") {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.split_once(':') {
        (from.trim(), to.trim())
    } else {
        (pair.trim(), "")
    }
}

pub fn execute_path_query<W: io::Write>(
    pair: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (from, to) = split_endpoint_pair(pair);
    if crate::navigate::find_symbols_by_name(from, ingest, interner).is_empty() {
        return Err(io::Error::other(format!(
            "ripwire: --path endpoint not found: {from}"
        )));
    }
    if crate::navigate::find_symbols_by_name(to, ingest, interner).is_empty() {
        return Err(io::Error::other(format!(
            "ripwire: --path endpoint not found: {to}"
        )));
    }
    let val = crate::mcp::graph_tools::call_path_between(ingest, graph, interner, from, to);
    crate::serialize::serialize_path_between_xml(&val, out)
}

pub fn execute_workspace_doc_query<W: io::Write>(
    ws: &TopWorkspaceArgs,
    ingest: &crate::domain::IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    if let Some(sym) = ws.whereis.as_deref() {
        let val = crate::mcp::doc_tools::call_whereis(ingest, interner, sym);
        crate::serialize::serialize_whereis_xml(&val, out)?;
    } else if ws.doc_drift {
        let val = crate::mcp::doc_tools::call_doc_drift(ingest, interner);
        crate::serialize::serialize_doc_drift_xml(&val, out)?;
    } else if ws.flags {
        let val = crate::mcp::doc_tools::call_flags(ingest, interner);
        crate::serialize::serialize_flags_xml(&val, out)?;
    } else if let Some(sym) = ws.mentions.as_deref() {
        if crate::navigate::find_symbols_by_name(sym, ingest, interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire: --mentions symbol not found: {sym}"
            )));
        }
        let val = crate::mcp::doc_tools::call_mentions(ingest, interner, sym);
        crate::serialize::serialize_mentions_xml(&val, out)?;
    }
    Ok(())
}

pub fn execute_retrieval_dispatch<W: io::Write>(
    ret: &TopRetrievalArgs,
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    pr: &crate::rank::PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    if let Some(task) = ret.task_for.as_deref() {
        let val = crate::mcp::retrieval::call_for_task(ingest, interner, &pr.ranks, task, None);
        crate::serialize::serialize_for_xml(&val, out)?;
    } else if let Some(pat) = ret.grep.as_deref() {
        let val = crate::mcp::retrieval::call_grep(ingest, interner, pat, None);
        crate::serialize::serialize_grep_xml(&val, out)?;
    } else if let Some(syms) = ret.connect.as_deref() {
        execute_connect_query(syms, ingest, graph, interner, out)?;
    } else if let Some(pair) = ret.path_between.as_deref() {
        execute_path_query(pair, ingest, graph, interner, out)?;
    } else if let Some(ex) = ret.exemplar.as_deref() {
        let val = crate::mcp::retrieval::call_exemplar(ingest, interner, &pr.ranks, ex);
        crate::serialize::serialize_exemplar_xml(root, &val, out)?;
    } else if let Some(iface) = ret.lego.as_deref() {
        let val = crate::mcp::retrieval::call_lego(ingest, interner, iface);
        crate::serialize::serialize_lego_xml(root, &val, out)?;
    } else if let Some(rec) = ret.recall.as_deref() {
        let val = crate::mcp::retrieval::call_memory_recall(ingest, interner, rec, None);
        crate::serialize::serialize_recall_xml(root, rec, &val, out)?;
    } else if let Some(tr) = ret.from_trace.as_deref() {
        let trace_text = std::fs::read_to_string(tr).unwrap_or_else(|_| tr.to_string());
        let val = crate::mcp::graph_tools::call_from_trace(ingest, interner, &trace_text);
        crate::serialize::serialize_from_trace_xml(&val, out)?;
    }
    Ok(())
}

fn dispatch_top_retrieval(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let ret = &cli.retrieval;
    if ret.task_for.is_none()
        && ret.grep.is_none()
        && ret.connect.is_none()
        && ret.path_between.is_none()
        && ret.exemplar.is_none()
        && ret.lego.is_none()
        && ret.recall.is_none()
        && ret.from_trace.is_none()
    {
        return None;
    }
    Some((|| {
        let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
        let mut stdout = io::stdout().lock();
        execute_retrieval_dispatch(ret, root, &ingest, &graph, &pr, &interner, &mut stdout)
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
        let mut stdout = io::stdout().lock();
        if ws.owners {
            let commits =
                crate::git::history::walk_repository_history(std::path::Path::new(root)).ok();
            let val = crate::mcp::doc_tools::call_owners(commits.as_deref(), None);
            crate::serialize::serialize_owners_xml(&val, &mut stdout)?;
        } else if ws.stray_content {
            let commits =
                crate::git::history::walk_repository_history(std::path::Path::new(root)).ok();
            let val = crate::mcp::doc_tools::call_stray_content(commits.as_deref());
            crate::serialize::serialize_stray_content_xml(&val, &mut stdout)?;
        } else {
            let (ingest, _graph, _pr, interner) = load_repo_analysis(root)?;
            execute_workspace_doc_query(ws, &ingest, &interner, &mut stdout)?;
        }
        Ok(())
    })())
}
fn extract_symbol_body<'a>(
    sym: &'a crate::domain::Symbol,
    ingest: &'a crate::domain::IngestResult,
    interner: &'a Interner,
    root_path: &std::path::Path,
) -> Option<(&'a crate::domain::Symbol, &'a str, &'a str, String)> {
    let name = interner.resolve(&sym.name);
    let rel_path = interner.resolve(&ingest.files[sym.file_id as usize].path);
    let bytes = std::fs::read(root_path.join(rel_path)).ok()?;
    let start = (sym.sig_start_byte as usize).min(bytes.len());
    let end = (sym.end_byte as usize).min(bytes.len());
    if start < end {
        let body = String::from_utf8_lossy(&bytes[start..end]).to_string();
        Some((sym, name, rel_path, body))
    } else {
        None
    }
}

fn execute_expand_query<W: io::Write>(
    root: &str,
    syms_spec: &str,
    ingest: &crate::domain::IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let sym_names: Vec<&str> = syms_spec
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut expanded_items = Vec::new();
    let root_path = std::path::Path::new(root);

    for sym_name in sym_names {
        let ids = crate::navigate::find_symbols_by_name(sym_name, ingest, interner);
        if ids.is_empty() {
            return Err(io::Error::other(format!(
                "ripwire: --expand symbol not found: {sym_name}"
            )));
        }
        for &id in &ids {
            if let Some(item) =
                extract_symbol_body(&ingest.symbols[id as usize], ingest, interner, root_path)
            {
                expanded_items.push(item);
            }
        }
    }

    let items_slice: Vec<(&crate::domain::Symbol, &str, &str, &str)> = expanded_items
        .iter()
        .map(|(s, n, p, b)| (*s, *n, *p, b.as_str()))
        .collect();

    crate::serialize::serialize_expand_xml(root, &items_slice, out)
}

fn execute_pack_top_n_query<W: io::Write>(
    root: &str,
    n: usize,
    ingest: &crate::domain::IngestResult,
    pr: &crate::rank::PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (file_order, buckets) = crate::serialize::bucket_symbols_by_rank(
        &ingest.symbols,
        ingest.files.len(),
        &pr.ranks,
        Some(n),
    );
    let root_path = std::path::Path::new(root);
    let mut bodies = Vec::new();
    let mut count = 0;

    for f in file_order {
        if count >= n {
            break;
        }
        let rel_path = interner.resolve(&ingest.files[f as usize].path);
        let file_disk_path = root_path.join(rel_path);
        if let Ok(bytes) = std::fs::read(&file_disk_path) {
            let content = String::from_utf8_lossy(&bytes).to_string();
            bodies.push((rel_path, content));
            count += buckets[f as usize].len();
        }
    }

    let bodies_slice: Vec<(&str, &str)> = bodies.iter().map(|(p, c)| (*p, c.as_str())).collect();
    crate::serialize::serialize_pack_top_n_xml(root, &bodies_slice, out)
}

fn execute_tree_query<W: io::Write>(
    root: &str,
    ingest: &crate::domain::IngestResult,
    pr: &crate::rank::PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (file_order, buckets) = crate::serialize::bucket_symbols_by_rank(
        &ingest.symbols,
        ingest.files.len(),
        &pr.ranks,
        None,
    );

    let mut file_symbols = Vec::new();
    for f in file_order {
        let rel_path = interner.resolve(&ingest.files[f as usize].path);
        let bucket = &buckets[f as usize];
        let mut top3 = Vec::new();
        for &sym_id in bucket.iter().take(3) {
            let sym = &ingest.symbols[sym_id as usize];
            top3.push((sym.kind.tag(), interner.resolve(&sym.name)));
        }
        file_symbols.push(crate::serialize::TreeFileSymbols {
            path: rel_path,
            total_symbols: bucket.len(),
            top_symbols: top3,
        });
    }
    crate::serialize::serialize_tree_xml(root, &file_symbols, pr.iterations, out)
}
fn dispatch_top_detail(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let detail = &cli.detail;
    if detail.expand.is_none()
        && detail.outline.is_none()
        && detail.pack_top_n.is_none()
        && !detail.tree
    {
        return None;
    }

    Some((|| {
        let (ingest, _graph, pr, interner) = load_repo_analysis(root)?;
        let mut stdout = io::stdout().lock();

        if let Some(syms) = detail.expand.as_deref().or(detail.outline.as_deref()) {
            execute_expand_query(root, syms, &ingest, &interner, &mut stdout)?;
        } else if let Some(n) = detail.pack_top_n {
            execute_pack_top_n_query(root, n, &ingest, &pr, &interner, &mut stdout)?;
        } else if detail.tree {
            execute_tree_query(root, &ingest, &pr, &interner, &mut stdout)?;
        }
        Ok(())
    })())
}
fn get_working_tree_changed_files(root: &str) -> Vec<String> {
    if let Ok(output) = std::process::Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(root)
        .output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        let files: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        if !files.is_empty() {
            return files;
        }
    }
    Vec::new()
}
pub fn execute_test_gate_query<W: io::Write>(
    root: &str,
    files_spec: Option<&str>,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    let files = match files_spec {
        Some(f) => f
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        None => get_working_tree_changed_files(root),
    };
    let specs: Vec<&str> = files.iter().map(String::as_str).collect();
    let report = crate::gate::affected::find_affected_tests(&specs, ingest, graph, interner);
    crate::serialize::serialize_test_gate_xml(root, &report.tests, report.reached_symbols, out)?;
    let has_obligations = !report.tests.is_empty() || report.reached_symbols > 0;
    Ok(has_obligations)
}

fn format_situ_list<W: io::Write>(
    out: &mut W,
    title: &str,
    items: Option<&Vec<serde_json::Value>>,
) -> io::Result<()> {
    let empty = Vec::new();
    let list = items.unwrap_or(&empty);
    writeln!(out, "  {title} ({})", list.len())?;
    for item in list {
        writeln!(out, "        {}", item.as_str().unwrap_or(""))?;
    }
    Ok(())
}

pub fn execute_situ_query<W: io::Write>(
    root: &str,
    files_spec: Option<&str>,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let files = match files_spec {
        Some(f) => f
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        None => get_working_tree_changed_files(root),
    };
    let commits = crate::git::history::walk_repository_history(std::path::Path::new(root)).ok();
    let val = crate::mcp::graph_tools::call_situational_awareness(
        ingest,
        graph,
        interner,
        &files,
        commits.as_deref(),
    );
    writeln!(
        out,
        "ripwire situational-awareness — {} changed file(s)",
        files.len()
    )?;
    writeln!(out, "root: {root}")?;
    writeln!(out, "  [1] blast radius: {} symbols", val["blast_radius"])?;
    format_situ_list(out, "[2] tests to run", val["tests_to_run"].as_array())?;
    format_situ_list(
        out,
        "[3] co-change — usually edited with these",
        val["forgotten_partners"].as_array(),
    )
}

pub fn execute_safe_delete_query<W: io::Write>(
    root: &str,
    sym_name: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let ids = crate::navigate::find_symbols_by_name(sym_name, ingest, interner);
    if ids.is_empty() {
        return Err(io::Error::other(format!(
            "ripwire: --safe-delete symbol not found: {sym_name}"
        )));
    }
    let sym_id = ids[0];
    let callers: Vec<(&str, &str)> = graph
        .callers(sym_id)
        .filter_map(|(cid, _)| {
            let s = ingest.symbols.get(cid as usize)?;
            let p = interner.resolve(&ingest.files[s.file_id as usize].path);
            let n = interner.resolve(&s.name);
            Some((p, n))
        })
        .collect();
    let ranks = vec![0.0; ingest.symbols.len()];
    let blast_radius =
        crate::navigate::transitive_impact(sym_id, ingest, graph, &ranks, interner, Some(32)).len();
    crate::serialize::serialize_safe_delete_xml(root, sym_name, &callers, blast_radius, out)
}

pub fn execute_exercises_query<W: io::Write>(
    root: &str,
    test_path: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let mut reached = Vec::new();
    for sym in &ingest.symbols {
        let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
        if !p.contains(test_path) {
            continue;
        }
        for (callee_id, _) in graph.callees(sym.id) {
            let Some(target) = ingest.symbols.get(callee_id as usize) else {
                continue;
            };
            let tp = interner.resolve(&ingest.files[target.file_id as usize].path);
            if !crate::gate::affected::is_test_path(tp) {
                reached.push((tp, interner.resolve(&target.name)));
            }
        }
    }
    crate::serialize::serialize_exercises_xml(root, test_path, &reached, out)
}

pub fn execute_safety_dispatch<W: io::Write>(
    s: &TopSafetyArgs,
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<Option<i32>> {
    if let Some(files_opt) = &s.test_gate {
        let has_failures =
            execute_test_gate_query(root, files_opt.as_deref(), ingest, graph, interner, out)?;
        if has_failures {
            return Ok(Some(4));
        }
    } else if let Some(files_opt) = &s.situ {
        execute_situ_query(root, files_opt.as_deref(), ingest, graph, interner, out)?;
    } else if let Some(sym) = s.safe_delete.as_deref() {
        execute_safe_delete_query(root, sym, ingest, graph, interner, out)?;
    } else if let Some(testfile) = s.exercises.as_deref() {
        execute_exercises_query(root, testfile, ingest, graph, interner, out)?;
    }
    Ok(None)
}

fn dispatch_top_safety(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let s = &cli.safety;
    if s.test_gate.is_none() && s.situ.is_none() && s.safe_delete.is_none() && s.exercises.is_none()
    {
        return None;
    }

    Some((|| {
        let (ingest, graph, _pr, interner) = load_repo_analysis(root)?;
        let mut stdout = io::stdout().lock();
        if let Some(code) =
            execute_safety_dispatch(s, root, &ingest, &graph, &interner, &mut stdout)?
        {
            std::process::exit(code);
        }
        Ok(())
    })())
}
pub fn execute_dead_code_query<W: io::Write>(
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let mut dead = Vec::new();
    for sym in &ingest.symbols {
        if (sym.kind == crate::domain::SymKind::Function
            || sym.kind == crate::domain::SymKind::Method)
            && graph.in_degree(sym.id) == 0
        {
            let n = interner.resolve(&sym.name);
            let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
            dead.push((n, sym.kind.tag(), p, sym.line));
        }
    }
    crate::serialize::serialize_dead_code_xml(root, &dead, out)
}

fn build_community_module<'a>(
    id: u32,
    nodes: &[crate::domain::NodeId],
    ingest: &'a crate::domain::IngestResult,
    interner: &'a Interner,
) -> crate::serialize::CommunityModule<'a> {
    let mut members = Vec::new();
    let mut top_dir = "";
    let mut top_label = "";
    for &nid in nodes {
        if let Some(s) = ingest.symbols.get(nid as usize) {
            let n = interner.resolve(&s.name);
            let p = interner.resolve(&ingest.files[s.file_id as usize].path);
            if top_dir.is_empty() {
                top_dir = p;
            }
            if top_label.is_empty() {
                top_label = n;
            }
            members.push((s.kind.tag(), n, p, s.line));
        }
    }
    crate::serialize::CommunityModule {
        id,
        size: members.len(),
        dir: top_dir,
        label: top_label,
        members,
    }
}

pub fn execute_communities_query<W: io::Write>(
    root: &str,
    target_id: Option<u32>,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let comms = graph.communities();
    let mut by_comm: std::collections::HashMap<u32, Vec<crate::domain::NodeId>> =
        std::collections::HashMap::new();
    for (node, &c) in comms.comm.iter().enumerate() {
        if target_id.is_none() || target_id == Some(c) {
            by_comm.entry(c).or_default().push(node as u32);
        }
    }

    let mut comm_ids: Vec<u32> = by_comm.keys().copied().collect();
    comm_ids.sort_unstable();

    let modules: Vec<crate::serialize::CommunityModule<'_>> = comm_ids
        .into_iter()
        .map(|id| build_community_module(id, &by_comm[&id], ingest, interner))
        .collect();

    crate::serialize::serialize_communities_xml(root, &modules, out)
}

/// Configuration for `--around` ego-graph queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AroundConfig {
    pub depth: u32,
    pub fanout: usize,
}

pub fn execute_around_query<W: io::Write>(
    root: &str,
    sym_name: &str,
    cfg: AroundConfig,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let ids = crate::navigate::find_symbols_by_name(sym_name, ingest, interner);
    if ids.is_empty() {
        return Err(io::Error::other(format!(
            "ripwire: --around symbol not found: {sym_name}"
        )));
    }
    let sym_id = ids[0];
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    visited.insert(sym_id);
    queue.push_back((sym_id, 0u32));

    let mut result_syms = Vec::new();

    while let Some((curr, curr_d)) = queue.pop_front() {
        if let Some(s) = ingest.symbols.get(curr as usize) {
            let n = interner.resolve(&s.name);
            let p = interner.resolve(&ingest.files[s.file_id as usize].path);
            result_syms.push((n, s.kind.tag(), p, s.line));
        }
        if curr_d >= cfg.depth {
            continue;
        }
        for (callee_id, _) in graph.callees(curr).take(cfg.fanout) {
            if visited.insert(callee_id) {
                queue.push_back((callee_id, curr_d + 1));
            }
        }
    }

    crate::serialize::serialize_around_xml(root, sym_name, cfg.depth, cfg.fanout, &result_syms, out)
}

pub fn execute_external_surface_query<W: io::Write>(
    root: &str,
    ingest: &crate::domain::IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let by_name = crate::resolve::index_symbols_by_name(&ingest.symbols);
    let mut ext_counts: std::collections::HashMap<(lasso::Spur, crate::domain::Lang), usize> =
        std::collections::HashMap::new();

    for r in &ingest.references {
        if !by_name.contains_key(&r.callee_name) {
            *ext_counts.entry((r.callee_name, r.lang)).or_default() += 1;
        }
    }

    let mut items: Vec<(&str, &str, usize)> = ext_counts
        .iter()
        .map(|(&(n, l), &c)| (interner.resolve(&n), l.tag(), c))
        .collect();
    items.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(b.0)));

    crate::serialize::serialize_external_surface_xml(root, &items, out)
}

pub fn execute_deps_query<W: io::Write>(
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let mut file_inc = vec![0usize; ingest.files.len()];
    let mut file_aff = vec![0usize; ingest.files.len()];

    for u in 0..graph.node_count() as crate::domain::NodeId {
        let uf = ingest.symbols[u as usize].file_id as usize;
        for (v, _) in graph.callees(u) {
            let vf = ingest.symbols[v as usize].file_id as usize;
            if uf != vf && uf < file_inc.len() && vf < file_aff.len() {
                file_inc[uf] += 1;
                file_aff[vf] += 1;
            }
        }
    }

    let mut file_deps = Vec::new();
    for (i, f) in ingest.files.iter().enumerate() {
        let p = interner.resolve(&f.path);
        file_deps.push((p, file_inc[i], file_aff[i]));
    }
    file_deps.sort_by_key(|a| std::cmp::Reverse(a.1 + a.2));

    crate::serialize::serialize_deps_xml(root, &file_deps, out)
}

fn dispatch_top_arch(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let a = &cli.arch;
    if !a.dead_code
        && !a.communities
        && a.community.is_none()
        && !a.deps
        && a.around.is_none()
        && !a.external_surface
    {
        return None;
    }

    Some((|| {
        let (ingest, graph, _pr, interner) = load_repo_analysis(root)?;
        let mut stdout = io::stdout().lock();

        if a.dead_code {
            execute_dead_code_query(root, &ingest, &graph, &interner, &mut stdout)?;
        } else if a.communities || a.community.is_some() {
            execute_communities_query(root, a.community, &ingest, &graph, &interner, &mut stdout)?;
        } else if let Some(sym) = a.around.as_deref() {
            let cfg = AroundConfig {
                depth: a.around_depth,
                fanout: a.around_fanout,
            };
            execute_around_query(root, sym, cfg, &ingest, &graph, &interner, &mut stdout)?;
        } else if a.external_surface {
            execute_external_surface_query(root, &ingest, &interner, &mut stdout)?;
        } else if a.deps {
            execute_deps_query(root, &ingest, &graph, &interner, &mut stdout)?;
        }
        Ok(())
    })())
}

fn run_batch_subquery<W: io::Write>(
    line: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    pr: &crate::rank::PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let Some((verb, arg)) = line.split_once(':') else {
        return Ok(());
    };
    let a = arg.trim();
    let mut sub_buf = Vec::new();
    match verb.trim() {
        "callers" => {
            let _ = execute_nav_symbol_query(
                a,
                NavMode::Callers,
                ingest,
                graph,
                &pr.ranks,
                interner,
                &mut sub_buf,
            );
        }
        "callees" => {
            let _ = execute_nav_symbol_query(
                a,
                NavMode::Callees,
                ingest,
                graph,
                &pr.ranks,
                interner,
                &mut sub_buf,
            );
        }
        "impact" => {
            let _ = execute_nav_symbol_query(
                a,
                NavMode::Impact,
                ingest,
                graph,
                &pr.ranks,
                interner,
                &mut sub_buf,
            );
        }
        "uses" => {
            let _ = execute_nav_symbol_query(
                a,
                NavMode::Uses,
                ingest,
                graph,
                &pr.ranks,
                interner,
                &mut sub_buf,
            );
        }
        "grep" => {
            let val = crate::mcp::retrieval::call_grep(ingest, interner, a, None);
            let _ = crate::serialize::serialize_grep_xml(&val, &mut sub_buf);
        }
        "for" => {
            let val = crate::mcp::retrieval::call_for_task(ingest, interner, &pr.ranks, a, None);
            let _ = crate::serialize::serialize_for_xml(&val, &mut sub_buf);
        }
        _ => {}
    }
    writeln!(out, "  <result query=\"{line}\">")?;
    out.write_all(&sub_buf)?;
    writeln!(out, "  </result>")
}

pub fn execute_batch_content<W: io::Write>(
    root: &str,
    content: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    pr: &crate::rank::PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let lines: Vec<&str> = content
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .take(16)
        .collect();

    writeln!(
        out,
        "<batch schema=\"ripwire.batch/v1\" root=\"{root}\" count=\"{}\">",
        lines.len()
    )?;
    for line in lines {
        run_batch_subquery(line, ingest, graph, pr, interner, out)?;
    }
    writeln!(out, "</batch>")
}

pub fn execute_batch_file<W: io::Write>(
    root: &str,
    file_path: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    pr: &crate::rank::PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let content = if file_path == "-" {
        let mut buf = String::new();
        io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
        buf
    } else {
        std::fs::read_to_string(file_path)?
    };
    execute_batch_content(root, &content, ingest, graph, pr, interner, out)
}

fn dispatch_top_flags(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    if let Some(file) = cli.modifiers.batch.as_deref() {
        return Some((|| {
            let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
            let mut stdout = io::stdout().lock();
            execute_batch_file(root, file, &ingest, &graph, &pr, &interner, &mut stdout)
        })());
    }

    dispatch_top_nav(cli, root)
        .or_else(|| dispatch_top_retrieval(cli, root))
        .or_else(|| dispatch_top_workspace(cli, root))
        .or_else(|| dispatch_top_detail(cli, root))
        .or_else(|| dispatch_top_safety(cli, root))
        .or_else(|| dispatch_top_arch(cli, root))
}

/// Execute CLI from parsed options.
pub fn run_cli_args(cli: Cli) -> io::Result<()> {
    let root = cli.path.as_deref().unwrap_or(".");
    if let Some(res) = dispatch_top_flags(&cli, root) {
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
        let out_file = html_opt.unwrap_or_else(|| "ripwire.html".to_string());
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

    let budget = compute_effective_budget(cli.pack_budget, cli.token_budget, cli.max_tokens);
    match &cli.command {
        Some(cmd) => dispatch_subcommand(cmd),
        None => dispatch_default_run(&cli, root, budget),
    }
}

fn dispatch_default_run(cli: &Cli, root: &str, budget: Option<usize>) -> io::Result<()> {
    let emit_legend = !cli.legend.eq_ignore_ascii_case("none");
    let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
    let mut stdout = io::stdout().lock();
    let args = RankedStreamArgs {
        root,
        rank_by: &cli.rank_by,
        pack_budget: budget,
        top_k: cli.top_k,
        emit_legend,
    };
    execute_stream_xml_ranked(&args, &ingest, &graph, &pr, &interner, &mut stdout)
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
            println!("ripwire gate: pre-commit verification gates (Wave 2 feature)");
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
            println!("ripwire metrics: code complexity and health metrics (Wave 2 feature)");
            Ok(())
        }
        Commands::Slice(_) => {
            println!("ripwire slice: ARISE variable dataflow slicing (Wave 3 feature)");
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
    println!("ripwire: wrote interactive HTML call graph to {out_file}");
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

        let run = Cli::try_parse_from([
            "rw",
            ".",
            "--top-k",
            "150",
            "--max-tokens",
            "4000",
            "--legend",
            "none",
        ])
        .unwrap();
        assert_eq!(run.path, Some(".".into()));
        assert_eq!(run.top_k, Some(150));
        assert_eq!(run.max_tokens, Some(4000));
        assert_eq!(run.legend, "none");
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
        assert!(!GateDispatchArgs::default().is_active());
        assert!(dispatch_gate_command(".", GateDispatchArgs::default()).is_none());
        let chk = |e, a, b, d| {
            GateDispatchArgs {
                edit_check: e,
                affected: a,
                quality_baseline: b,
                quality_delta: d,
            }
            .is_active()
        };
        assert!(chk(Some("m"), None, false, false) && chk(None, Some("f"), false, false));
        assert!(chk(None, None, true, false) && chk(None, None, false, true));
    }

    #[test]
    fn test_compute_effective_budget() {
        assert_eq!(compute_effective_budget(Some(1024), None, None), Some(1024));
        assert_eq!(compute_effective_budget(None, Some(100), None), Some(400));
        assert_eq!(compute_effective_budget(None, None, Some(50)), Some(200));
        assert_eq!(
            compute_effective_budget(None, None, None),
            Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES)
        );
        assert_eq!(split_endpoint_pair("a,b"), ("a", "b"));
        assert_eq!(split_endpoint_pair("a:b"), ("a", "b"));
        assert_eq!(split_endpoint_pair("a->b"), ("a", "b"));
        assert_eq!(split_endpoint_pair("single"), ("single", ""));
    }
}
