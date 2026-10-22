//! MCP and LSP servers: `--mcp`, `--lsp`, `--listen`, `--mcp-token`, `--mcp-tools` and the `mcp`
//! subcommand. `--listen`, `--mcp-token` and `--mcp-tools` are parsed only.
use super::{Command, SharedOption, decode_subcommand};
use crate::cli::Cli;
use clap::{Args, Command as ClapCommand};
use std::io;
use std::path::Path;

/// Top-level MCP and LSP server flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct McpFlags {
    /// Launch stdio JSON-RPC Model Context Protocol server
    #[arg(long = "mcp")]
    pub mcp: bool,
    /// Enable LSP server bridge
    #[arg(long = "lsp")]
    pub lsp: bool,
    /// Run daemon listener
    #[arg(long = "listen")]
    pub listen: bool,
    /// Require authentication token for MCP transport
    #[arg(long = "mcp-token", value_name = "TOKEN")]
    pub mcp_token: Option<String>,
    /// Restrict exposed MCP tools to comma-separated list or profile (core|full)
    #[arg(long = "mcp-tools", value_name = "LIST")]
    pub mcp_tools: Option<String>,
}

/// `mcp` subcommand: serve MCP over stdio, optionally pinning a workspace root.
#[derive(Args, Debug, Default, PartialEq)]
pub struct McpArgs {
    /// Target workspace root to pin
    #[arg(value_name = "PATH")]
    pub path: Option<String>,
}

pub struct Mcp;
pub static MCP: Mcp = Mcp;

impl Command for Mcp {
    fn name(&self) -> &'static str {
        "mcp"
    }

    fn options(&self) -> &'static [SharedOption] {
        &[]
    }

    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        McpFlags::augment_args(cmd)
    }

    fn subcommands(&self) -> Vec<ClapCommand> {
        vec![
            McpArgs::augment_args(ClapCommand::new("mcp"))
                .about("Launch stdio JSON-RPC Model Context Protocol server"),
        ]
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let flags = cli.plugins.decode::<McpFlags>()?;
        if flags.mcp {
            return Some(crate::mcp::run_mcp_stdio_with_scip(
                root,
                cli.scip.as_deref().map(Path::new),
                crate::commands::tools(),
            ));
        }
        if flags.lsp {
            return Some(crate::mcp::run_lsp_stdio(root));
        }
        None
    }

    fn run_subcommand(
        &self,
        cli: &Cli,
        name: &str,
        matches: &clap::ArgMatches,
        root: &str,
    ) -> Option<io::Result<()>> {
        if name != "mcp" {
            return None;
        }
        Some(decode_subcommand::<McpArgs>(matches).and_then(|args| {
            crate::mcp::run_mcp_stdio_with_scip(
                args.path.as_deref().unwrap_or(root),
                cli.scip.as_deref().map(Path::new),
                crate::commands::tools(),
            )
        }))
    }
}
