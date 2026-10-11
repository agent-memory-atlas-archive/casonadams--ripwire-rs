use crate::domain::Interner;
use crate::graph::Graph;
use crate::mcp::run_mcp_stdio;
use crate::navigate::{find_symbols_by_name, navigate_1hop, transitive_impact};
use crate::rank::PageRankResult;
use crate::serialize::{serialize_xml, SerializeOptions};
use clap::{Args, Parser, Subcommand};
use std::io;

/// CLI command line options.
#[derive(Parser, Debug, Default, PartialEq)]
#[command(
    name = "rwire",
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

    /// Workflow subcommand group
    #[command(subcommand)]
    pub command: Option<Commands>,
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

    /// Measure quality delta against baseline
    #[arg(long)]
    pub quality_delta: bool,

    /// Show test files affected by changes
    #[arg(long)]
    pub affected: Option<String>,
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

pub fn execute_stream_xml_with<W: io::Write>(
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    serialize_xml(
        ingest,
        graph,
        pr,
        interner,
        out,
        &SerializeOptions {
            root: root.to_string(),
            top_k: None,
            include_pr: true,
        },
    )
}

fn execute_stream_xml(root: &str) -> io::Result<()> {
    let state = crate::mcp::McpState::load(root)
        .ok_or_else(|| io::Error::other("failed to load repository"))?;
    let mut stdout = io::stdout().lock();
    execute_stream_xml_with(
        root,
        &state.ingest,
        &state.graph,
        &state.pr,
        &state.interner,
        &mut stdout,
    )
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
        writeln!(
            out,
            "Impacted callers for '{}' ({}):",
            sym_name,
            impact.len()
        )?;
        for node in impact {
            writeln!(
                out,
                "  [{}] {} ({}:{})",
                node.hops, node.name, node.path, node.line
            )?;
        }
        return Ok(());
    }
    let Some(nav) = navigate_1hop(id, ctx.ingest, ctx.graph, ctx.ranks, ctx.interner) else {
        return Ok(());
    };
    if args.callers {
        writeln!(
            out,
            "Direct callers of '{}' ({}):",
            sym_name,
            nav.callers.len()
        )?;
        for c in nav.callers {
            writeln!(out, "  {} ({}:{})", c.name, c.path, c.line)?;
        }
    } else if args.callees {
        writeln!(
            out,
            "Direct callees of '{}' ({}):",
            sym_name,
            nav.callees.len()
        )?;
        for c in nav.callees {
            writeln!(out, "  {} ({}:{})", c.name, c.path, c.line)?;
        }
    } else {
        writeln!(
            out,
            "Symbol '{}' ({}:{}): {} callers, {} callees",
            nav.target.name,
            nav.target.path,
            nav.target.line,
            nav.callers.len(),
            nav.callees.len()
        )?;
    }
    Ok(())
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
    writeln!(out, "<cochange pairs=\"{}\">", pairs.len())?;
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

fn dispatch_gate_command(
    root: &str,
    edit_check: Option<&str>,
    affected: Option<&str>,
) -> Option<io::Result<()>> {
    if let Some(sym) = edit_check {
        let state = match crate::mcp::McpState::load(root) {
            Some(s) => s,
            None => return Some(Err(io::Error::other("failed to load repository"))),
        };
        let mut stdout = io::stdout().lock();
        return Some(execute_edit_check_with(sym, &state, &mut stdout));
    }
    if let Some(spec) = affected {
        let state = match crate::mcp::McpState::load(root) {
            Some(s) => s,
            None => return Some(Err(io::Error::other("failed to load repository"))),
        };
        let mut stdout = io::stdout().lock();
        return Some(execute_affected_with(spec, &state, &mut stdout));
    }
    None
}

/// Execute CLI from parsed options.
pub fn run_cli_args(cli: Cli) -> io::Result<()> {
    let root = cli.path.as_deref().unwrap_or(".");
    if let Some(res) = dispatch_git_metrics_command(
        root,
        cli.cochange.as_ref().map(|o| o.as_deref()),
        cli.hotspots,
        cli.since.as_deref(),
    ) {
        return res;
    }
    if let Some(res) =
        dispatch_gate_command(root, cli.edit_check.as_deref(), cli.affected.as_deref())
    {
        return res;
    }

    match &cli.command {
        Some(Commands::Mcp(mcp_args)) => {
            let root = mcp_args.path.as_deref().unwrap_or(".");
            run_mcp_stdio(root)
        }
        Some(Commands::Navigate(nav_args)) => execute_navigate(nav_args),
        Some(Commands::Orient(orient_args)) => {
            let root = orient_args.path.as_deref().unwrap_or(".");
            execute_stream_xml(root)
        }
        Some(Commands::Gate(gate_args)) => {
            let root = gate_args.path.as_deref().unwrap_or(".");
            if let Some(res) = dispatch_gate_command(
                root,
                gate_args.edit_check.as_deref(),
                gate_args.affected.as_deref(),
            ) {
                return res;
            }
            println!("rwire gate: pre-commit verification gates (Wave 2 feature)");
            Ok(())
        }
        Some(Commands::Metrics(metrics_args)) => {
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
        Some(Commands::Slice(_)) => {
            println!("rwire slice: ARISE variable dataflow slicing (Wave 3 feature)");
            Ok(())
        }
        None => execute_stream_xml(root),
    }
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
        let help = Cli::try_parse_from(["rwire", "--help"]);
        assert!(help.is_err()); // --help exits with help message

        let default_run = Cli::try_parse_from(["rwire", "."]).unwrap();
        assert_eq!(default_run.path, Some(".".into()));
        assert!(default_run.command.is_none());

        let mcp_cmd = Cli::try_parse_from(["rwire", "mcp"]).unwrap();
        assert_eq!(mcp_cmd.command, Some(Commands::Mcp(McpArgs::default())));

        let nav_cmd =
            Cli::try_parse_from(["rwire", "navigate", "--symbol", "main", "--callers"]).unwrap();
        assert!(matches!(nav_cmd.command, Some(Commands::Navigate(_))));

        let orient_cmd = Cli::try_parse_from(["rwire", "orient", "--for", "auth"]).unwrap();
        assert!(matches!(orient_cmd.command, Some(Commands::Orient(_))));
    }
}
