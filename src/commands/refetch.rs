//! `--refetch HANDLE`: expands a compact handle from `--handles` into the symbol's source.
//! A bare `--refetch` clones a remote URL root fresh instead.
use super::common::mcp::mcp_tool;
use super::common::retrieval::report_query;
use super::{Command, Run};
use crate::cli::Cli;
use crate::query::commands::{CommandDescriptor, QueryContext, render_xml};
use crate::serialize::JsonValueExt;
use clap::{Args, Command as ClapCommand};
use serde_json::Value;
use std::io::{self, Write};

/// `--refetch`.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct RefetchArgs {
    /// Expand a compact handle returned by --handles; bare, clone a remote URL root fresh
    #[arg(long = "refetch", value_name = "HANDLE", num_args = 0..=1, default_missing_value = "")]
    pub refetch: Option<String>,
}

impl RefetchArgs {
    fn handle(&self) -> Option<&str> {
        self.refetch.as_deref().filter(|handle| !handle.is_empty())
    }
}

pub fn execute<W: Write>(query: &QueryContext<'_>, handle: &str, out: &mut W) -> io::Result<()> {
    let request = crate::query::detail::BodyRequest {
        root: query.root,
        symbols: handle,
        path_hint: None,
        start_line: None,
        end_line: None,
        outline: false,
        compress: false,
    };
    crate::query::detail::execute_body_query(&request, query.ingest, query.interner, query.sources, out)
}

pub struct Refetch;
pub static REFETCH: Refetch = Refetch;

impl Command for Refetch {
    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        RefetchArgs::augment_args(cmd)
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let args = cli.plugins.decode::<RefetchArgs>()?;
        let handle = args.handle()?;
        Some(report_query(&Run::new(cli, root, &[]), |query, out| {
            execute(query, handle, out)
        }))
    }

    fn tools(&self) -> &'static [&'static CommandDescriptor] {
        TOOLS
    }

    fn refreshes_remote_checkout(&self, cli: &Cli) -> bool {
        cli.plugins
            .decode::<RefetchArgs>()
            .is_some_and(|args| args.refetch.as_deref() == Some(""))
    }
}

fn call_body(context: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let request = crate::query::detail::BodyRequest {
        root: context.root,
        symbols: args.req_str("symbol")?,
        path_hint: args.opt_str("path"),
        start_line: args.opt_usize("start_line"),
        end_line: args.opt_usize("end_line"),
        outline: args.bool_or("outline", false),
        compress: args.bool_or("compress", false),
    };
    render_xml(|out| {
        crate::query::detail::execute_body_query(
            &request,
            context.ingest,
            context.interner,
            context.sources,
            out,
        )
    })
}

static TOOLS: &[&CommandDescriptor] = &[mcp_tool!(
    "fetch_body",
    "Full (or partial-range) source of a symbol's definition",
    &[
        ("symbol", "string", "Symbol name"),
        ("path", "string", "Optional file path hint"),
        ("start_line", "integer", "Optional 1-based start line"),
        ("end_line", "integer", "Optional 1-based end line"),
        ("outline", "boolean", "Show definition skeleton"),
        ("compress", "boolean", "Compress implementation source"),
    ],
    &["symbol"],
    call_body
)];
