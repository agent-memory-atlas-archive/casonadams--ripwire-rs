//! Symbol navigation: `--callers`, `--callees`, `--impact`, `--uses`, `--find-symbol` and their MCP tools.
use super::Command;
use crate::cli::{Cli, OutputDialect, load_repo_analysis_for_cli};
use crate::query::commands::{CommandDescriptor, QueryContext, render_xml};
use crate::query::navigation::{NavContext, NavMode, execute_nav_symbol_query_in_root};
use crate::serialize::JsonValueExt;
use clap::{Args, Command as ClapCommand};
use serde_json::Value;
use std::io;

/// Top-level symbol navigation flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct NavArgs {
    /// Query 1-hop callers of a symbol
    #[arg(long, value_name = "SYM")]
    pub callers: Option<String>,
    /// Query 1-hop callees of a symbol
    #[arg(long, value_name = "SYM")]
    pub callees: Option<String>,
    /// Query transitive impact blast radius of a symbol
    #[arg(long, value_name = "SYM")]
    pub impact: Option<String>,
    /// Limit transitive impact traversal to N call hops
    #[arg(long = "impact-depth", global = true, value_name = "N")]
    pub impact_depth: Option<u32>,
    /// Query all static use-sites of a symbol
    #[arg(long, value_name = "SYM")]
    pub uses: Option<String>,
    /// Query 1-hop callers and callees of a symbol
    #[arg(long = "find-symbol", value_name = "SYM")]
    pub find_symbol: Option<String>,
}

impl NavArgs {
    fn active_query(&self) -> Option<(&str, NavMode)> {
        self.callers
            .as_deref()
            .map(|s| (s, NavMode::Callers))
            .or_else(|| self.callees.as_deref().map(|s| (s, NavMode::Callees)))
            .or_else(|| self.impact.as_deref().map(|s| (s, NavMode::Impact)))
            .or_else(|| self.uses.as_deref().map(|s| (s, NavMode::Uses)))
            .or_else(|| self.find_symbol.as_deref().map(|s| (s, NavMode::FindSymbol)))
    }
}

/// `--impact-depth`, shared with the `navigate` subcommand.
#[must_use]
pub fn impact_depth(cli: &Cli) -> Option<u32> {
    cli.plugins.decode::<NavArgs>()?.impact_depth
}

pub struct Nav;
pub static NAV: Nav = Nav;

impl Command for Nav {
    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        NavArgs::augment_args(cmd)
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let args = cli.plugins.decode::<NavArgs>()?;
        let (symbol, mode) = args.active_query()?;
        Some((|| {
            let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root, false)?;
            let mut stdout_lock = io::stdout().lock();
            let mut stdout = cli.wrap_xml_writer(&mut stdout_lock, false);
            let context = NavContext {
                ingest: &ingest,
                graph: &graph,
                ranks: &pr.ranks,
                pr_iters: Some(pr.iterations),
                interner: &interner,
                page: crate::serialize::PageRequest {
                    limit: cli.modifiers.limit,
                    offset: cli.modifiers.offset,
                },
                max_depth: args.impact_depth,
            };
            execute_nav_symbol_query_in_root(root, symbol, mode, &context, &mut stdout)?;
            stdout.finish()
        })())
    }

    fn tools(&self) -> &'static [&'static CommandDescriptor] {
        TOOLS
    }
}

fn call_navigation(state: &QueryContext<'_>, args: &Value, mode: NavMode) -> Result<Value, String> {
    let symbol = args.req_str("symbol")?;
    let context = NavContext {
        ingest: state.ingest,
        graph: state.graph,
        ranks: &state.pr.ranks,
        pr_iters: Some(state.pr.iterations),
        interner: state.interner,
        page: crate::serialize::PageRequest {
            limit: args.opt_usize("limit"),
            offset: args.opt_usize("offset"),
        },
        max_depth: args.opt_usize("max_depth").map(|depth| depth as u32),
    };
    render_xml(|out| execute_nav_symbol_query_in_root(state.root, symbol, mode, &context, out))
}

macro_rules! nav_tool {
    ($name:expr, $desc:expr, $symbol_desc:expr, $mode:expr) => {
        &CommandDescriptor {
            name: $name,
            description: $desc,
            params: &[
                ("symbol", "string", $symbol_desc),
                ("limit", "integer", "Maximum report items"),
                ("offset", "integer", "Report item offset"),
                ("max_depth", "integer", "Maximum traversal depth"),
            ],
            required: &["symbol"],
            handler: |state, args| call_navigation(state, args, $mode),
        }
    };
}

static TOOLS: &[&CommandDescriptor] = &[
    nav_tool!(
        "callees",
        "Direct (1-hop) callees of a symbol",
        "Symbol name to look up",
        NavMode::Callees
    ),
    nav_tool!(
        "callers",
        "Direct (1-hop) callers of a symbol (alias for find_referencing_symbols)",
        "Symbol name to look up",
        NavMode::Callers
    ),
    nav_tool!(
        "find_referencing_symbols",
        "Direct (1-hop) callers of a symbol",
        "Symbol name to look up",
        NavMode::Callers
    ),
    nav_tool!(
        "find_symbol",
        "A symbol's 1-hop neighborhood: the symbol plus direct callers and callees",
        "Symbol name to look up",
        NavMode::FindSymbol
    ),
    nav_tool!(
        "impact",
        "Transitive blast radius of a symbol via calls, ranked by PageRank",
        "Symbol name to analyze blast radius for",
        NavMode::Impact
    ),
    nav_tool!(
        "uses",
        "All statically resolvable use-sites of a symbol: call, read, write, import, extends",
        "Symbol name",
        NavMode::Uses
    ),
];
