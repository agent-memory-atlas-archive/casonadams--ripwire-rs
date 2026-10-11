use crate::crawler::{crawl_directory, CrawlerOptions};
use crate::domain::Interner;
use crate::graph::Graph;
use crate::ingest::{ingest_crawled_files, LanguageRegistry};
use crate::mcp::run_mcp_stdio;
use crate::navigate::{find_symbols_by_name, navigate_1hop, transitive_impact};
use crate::rank::PageRankConfig;
use crate::serialize::{serialize_xml, SerializeOptions};
use clap::{Args, Parser, Subcommand};
use std::io;
use std::path::Path;

/// CLI command line options.
#[derive(Parser, Debug)]
#[command(
    name = "rwire",
    version,
    about = "High-throughput agent context router and static analysis engine"
)]
pub struct Cli {
    /// Target repository path (default: current directory)
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

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

fn execute_stream_xml(root: &str) -> io::Result<()> {
    let files = crawl_directory(Path::new(root), &CrawlerOptions::default());
    let registry = LanguageRegistry::new_tier1().map_err(|e| io::Error::other(e.to_string()))?;
    let interner = Interner::new();
    let ingest = ingest_crawled_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&PageRankConfig::default());

    let mut stdout = io::stdout().lock();
    serialize_xml(
        &ingest,
        &graph,
        &pr,
        &interner,
        &mut stdout,
        &SerializeOptions {
            root: root.to_string(),
            top_k: None,
            include_pr: true,
        },
    )
}

fn print_symbol_nav(
    id: crate::domain::NodeId,
    args: &NavigateArgs,
    sym_name: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
) {
    if args.impact {
        let impact = transitive_impact(id, ingest, graph, ranks, interner, None);
        println!("Impacted callers for '{}' ({}):", sym_name, impact.len());
        for node in impact {
            println!(
                "  [{}] {} ({}:{})",
                node.hops, node.name, node.path, node.line
            );
        }
        return;
    }
    let Some(nav) = navigate_1hop(id, ingest, graph, ranks, interner) else {
        return;
    };
    if args.callers {
        println!("Direct callers of '{}' ({}):", sym_name, nav.callers.len());
        for c in nav.callers {
            println!("  {} ({}:{})", c.name, c.path, c.line);
        }
    } else if args.callees {
        println!("Direct callees of '{}' ({}):", sym_name, nav.callees.len());
        for c in nav.callees {
            println!("  {} ({}:{})", c.name, c.path, c.line);
        }
    } else {
        println!(
            "Symbol '{}' ({}:{}): {} callers, {} callees",
            nav.target.name,
            nav.target.path,
            nav.target.line,
            nav.callers.len(),
            nav.callees.len()
        );
    }
}

fn execute_navigate(args: &NavigateArgs) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(".");
    let files = crawl_directory(Path::new(root), &CrawlerOptions::default());
    let registry = LanguageRegistry::new_tier1().map_err(|e| io::Error::other(e.to_string()))?;
    let interner = Interner::new();
    let ingest = ingest_crawled_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&PageRankConfig::default());

    let sym_name = match args.symbol.as_deref() {
        Some(s) => s,
        None => {
            eprintln!("Error: --symbol is required for navigate commands");
            return Ok(());
        }
    };

    let ids = find_symbols_by_name(sym_name, &ingest, &interner);
    if ids.is_empty() {
        println!("Symbol '{}' not found", sym_name);
        return Ok(());
    }

    for id in ids {
        print_symbol_nav(id, args, sym_name, &ingest, &graph, &pr.ranks, &interner);
    }
    Ok(())
}

/// Execute CLI from parsed options.
pub fn run_cli_args(cli: Cli) -> io::Result<()> {
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
        Some(Commands::Gate(_)) => {
            println!("rwire gate: pre-commit verification gates (Wave 2 feature)");
            Ok(())
        }
        Some(Commands::Metrics(_)) => {
            println!("rwire metrics: code complexity and health metrics (Wave 2 feature)");
            Ok(())
        }
        Some(Commands::Slice(_)) => {
            println!("rwire slice: ARISE variable dataflow slicing (Wave 3 feature)");
            Ok(())
        }
        None => {
            let root = cli.path.as_deref().unwrap_or(".");
            execute_stream_xml(root)
        }
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
