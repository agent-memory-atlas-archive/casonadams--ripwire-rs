//! Retrieval ladder: `--for`, `--grep`, `--connect`, `--path-between`, `--exemplar`, `--lego`, `--recall`,
//! `--from-trace`, `--at`, `--verify`, `--pack-task`, `--graph-query`, `--match`, `--pattern`, `--regex`,
//! `--help-task`, `--refetch` and their MCP tools.
mod execute;
mod tools;
mod orient;

pub use execute::execute_retrieval_dispatch;
pub use orient::OrientArgs;

use super::{Command, decode_subcommand};
use crate::cli::Cli;
use crate::commands::context::{CliVerb, ExecutionContext, VerbGroup, run_cli_verb};
use crate::query::commands::{CommandDescriptor, QueryContext};
use clap::{Args, Command as ClapCommand};
use std::io::{self, Write};

/// Top-level retrieval & architecture flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct RetrievalArgs {
    /// Importance-weighted detail: full bodies for top N symbols, or lift collapsed rows
    #[arg(long = "detail", num_args = 0..=1, default_missing_value = "1", value_name = "N")]
    pub detail: Option<usize>,
    /// Cut returned set at the relevance cliff instead of fixed top-k
    #[arg(long = "adaptive")]
    pub adaptive: bool,
    /// Let --doc-drift and --whereis consult git history for deleted names
    #[arg(long = "with-history")]
    pub with_history: bool,
    /// Skip query-shape router and force plain subtoken+body BM25
    #[arg(long = "no-route")]
    pub no_route: bool,
    /// Disable query-mention anchor score lifting
    #[arg(long = "no-mention-boost")]
    pub no_mention_boost: bool,
    /// Disable doc-mention backtick surfacing
    #[arg(long = "no-doc-mention")]
    pub no_doc_mention: bool,
    /// Restore automatic body walk on the conceptual route
    #[arg(long = "auto-bodies")]
    pub auto_bodies: bool,
    /// Opt in to full lego and compose sections (lego,compose)
    #[arg(long = "sections", value_name = "SECTIONS")]
    pub sections: Option<String>,
    #[arg(long = "pack-signatures")]
    pub pack_signatures: bool,
    #[arg(long = "for", visible_alias = "query", value_name = "TASK")]
    pub task_for: Option<String>,
    /// Literal trigram text search
    #[arg(long, value_name = "PAT")]
    pub grep: Option<String>,
    /// Span tier a --grep hit must sit in: code (default) or any
    #[arg(long = "grep-in", value_name = "code|any")]
    pub grep_in: Option<String>,
    /// Where --and/--not terms must appear: line (default) or file
    #[arg(long = "grep-scope", value_name = "line|file")]
    pub grep_scope: Option<String>,
    /// Keep only --grep hits where PAT is also present (repeatable)
    #[arg(long = "and", value_name = "PAT")]
    pub and_pat: Vec<String>,
    /// Drop --grep hits where PAT is present (repeatable)
    #[arg(long = "not", value_name = "PAT")]
    pub not_pat: Vec<String>,
    /// Enclosing symbol or directory filter
    #[arg(long = "in", value_name = "SCOPE")]
    pub in_scope: Option<String>,
    /// Emit compact refetch handles on retrieval rows
    #[arg(long = "handles")]
    pub handles: bool,
    /// Expand a compact handle returned by --handles; bare, clone a remote URL root fresh
    #[arg(long = "refetch", value_name = "HANDLE", num_args = 0..=1, default_missing_value = "")]
    pub refetch: Option<String>,
    /// Recommend the best ripwire verb for a natural-language task
    #[arg(long = "help-task", value_name = "TASK")]
    pub help_task: Option<String>,
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
    /// Resolve innermost symbol chain enclosing FILE:LINE
    #[arg(long = "at", value_name = "FILE:LINE")]
    pub at: Option<String>,
    /// Formally verify a code relationship claim (e.g. calls(A,B))
    #[arg(long = "verify", value_name = "CLAIM")]
    pub verify: Option<String>,
    /// Package single-call orientation task bundle under token budget
    #[arg(long = "pack-task", value_name = "TASK")]
    pub pack_task: Option<String>,
    /// Evaluate small functional graph query DSL expression
    #[arg(long = "graph-query", value_name = "EXPR")]
    pub graph_query: Option<String>,
    /// AST pattern query expression
    #[arg(long = "match", value_name = "QUERY")]
    pub ast_match: Option<String>,
    /// Code-shaped structural pattern search
    #[arg(long = "pattern", value_name = "PAT")]
    pub pattern: Option<String>,
    /// Regular expression search
    #[arg(long = "regex", value_name = "PAT")]
    pub regex: Option<String>,
    /// Emit only signatures without bodies
    #[arg(long = "signatures-only")]
    pub signatures_only: bool,
    /// Connecting radius for --connect
    #[arg(long = "connect-radius", value_name = "N")]
    pub connect_radius: Option<u32>,
    /// Lines before grep hit
    #[arg(long = "grep-before", value_name = "N")]
    pub grep_before: Option<usize>,
    /// Lines after grep hit
    #[arg(long = "grep-after", value_name = "N")]
    pub grep_after: Option<usize>,
    /// Lines around grep hit
    #[arg(long = "grep-context", value_name = "N")]
    pub grep_context: Option<usize>,
}

impl VerbGroup for RetrievalArgs {
    fn is_group_active(&self) -> bool {
        self.task_for.is_some()
            || self.grep.is_some()
            || self.connect.is_some()
            || self.path_between.is_some()
            || self.exemplar.is_some()
            || self.lego.is_some()
            || self.recall.is_some()
            || self.from_trace.is_some()
            || self.at.is_some()
            || self.verify.is_some()
            || self.pack_task.is_some()
            || self.graph_query.is_some()
            || self.ast_match.is_some()
            || self.pattern.is_some()
            || self.regex.is_some()
            || self.help_task.is_some()
            || self.refetch.as_deref().is_some_and(|handle| !handle.is_empty())
            || self.pack_signatures
    }
}

impl RetrievalArgs {
    fn validate(&self) -> io::Result<()> {
        let Some(raw) = self.sections.as_deref() else {
            return Ok(());
        };
        if self.task_for.is_none() {
            return Err(io::Error::other(
                "ripwire-rs: --sections= modifies --for=TASK; pass it too",
            ));
        }
        crate::query::retrieval::parse_for_sections(raw)
            .map(|_| ())
            .map_err(io::Error::other)
    }
}

impl CliVerb for RetrievalArgs {
    fn execute_verb<W: Write>(
        &self,
        ctx: &ExecutionContext<'_>,
        out: &mut crate::serialize::XmlWriter<'_, W>,
    ) -> io::Result<()> {
        if let Some(task) = self.pack_task.as_deref()
            && let Some(part_opt) = crate::commands::safety::args(ctx.cli).partition
        {
            return crate::navigate::execute_pack_task_partitioned(
                task,
                ctx.root,
                part_opt.unwrap_or(2),
                ctx.ingest,
                ctx.graph,
                ctx.interner,
                out,
            );
        }
        if let Some(task) = self.recall.as_deref() {
            let inputs = crate::query::search::collect_recall_inputs(std::path::Path::new(ctx.root));
            let val = crate::query::search::call_memory_recall(
                &inputs,
                task,
                ctx.cli.top_k,
                ctx.cli.token_budget.or(ctx.cli.max_tokens),
            );
            return crate::serialize::serialize_recall_text(task, &val, out);
        }
        if let Some(expr) = self.graph_query.as_deref() {
            return crate::navigate::execute_graph_query_expr_paged(
                expr,
                ctx.root,
                ctx.ingest,
                ctx.graph,
                ctx.interner,
                crate::serialize::PageRequest {
                    limit: ctx.cli.modifiers.limit,
                    offset: ctx.cli.modifiers.offset,
                },
                out,
            );
        }
        execute::execute_retrieval_verb(self, ctx, out)
    }

    fn emits_plain_text(&self) -> bool {
        self.recall.is_some()
    }
}

/// `--detail` budget shared by the owner and retrieval renderers.
#[must_use]
pub fn detail(cli: &Cli) -> Option<usize> {
    cli.plugins.decode::<RetrievalArgs>()?.detail
}

/// Whether `--with-history` is set.
#[must_use]
pub fn with_history(cli: &Cli) -> bool {
    cli.plugins
        .decode::<RetrievalArgs>()
        .is_some_and(|args| args.with_history)
}


/// `ripwire-rs --orient --for=TASK` bundle, shaped by the `--for` flags on the same command line.
#[must_use]
pub fn orient_for_task(
    cli: &Cli,
    query: &QueryContext<'_>,
    task: &str,
) -> serde_json::Value {
    let args: RetrievalArgs = cli.plugins.decode().unwrap_or_default();
    crate::query::retrieval::call_for_task_with_options(
        query.ingest,
        query.interner,
        &query.pr.ranks,
        task,
        args.for_options(query, cli.token_budget.or(cli.max_tokens)),
    )
}

pub struct Retrieval;
pub static RETRIEVAL: Retrieval = Retrieval;

impl Command for Retrieval {
    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        RetrievalArgs::augment_args(cmd)
    }

    fn validate(&self, cli: &Cli) -> io::Result<()> {
        cli.plugins
            .decode::<RetrievalArgs>()
            .map_or(Ok(()), |args| args.validate())
    }

    fn refreshes_remote_checkout(&self, cli: &Cli) -> bool {
        cli.plugins
            .decode::<RetrievalArgs>()
            .is_some_and(|args| args.refetch.as_deref() == Some(""))
    }

    fn subcommands(&self) -> Vec<ClapCommand> {
        vec![
            OrientArgs::augment_args(ClapCommand::new("orient"))
                .about("Exploration and task-based context orientation"),
        ]
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let args = cli.plugins.decode::<RetrievalArgs>()?;
        run_cli_verb(&args, cli, root)
    }

    fn run_subcommand(
        &self,
        cli: &Cli,
        name: &str,
        matches: &clap::ArgMatches,
        root: &str,
    ) -> Option<io::Result<()>> {
        if name != "orient" {
            return None;
        }
        Some(
            decode_subcommand::<OrientArgs>(matches)
                .and_then(|args| orient::execute_orient_subcommand(cli, &args, root)),
        )
    }

    fn tools(&self) -> &'static [&'static CommandDescriptor] {
        tools::TOOLS
    }
}
