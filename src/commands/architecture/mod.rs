//! Architecture group: `--dead-code`, `--communities`, `--around`, `--deps`, `--zoom`, `--mermaid`,
//! `--arch`, `--metrics`, `--ensemble`, `--quality-panel`, `--layout`, `--field-affinity`, plus the MCP
//! tools `analyze`, `explore`, and `rank_by`.

mod execute;
mod tools;

pub use execute::*;

use super::Command;
use crate::cli::Cli;
use crate::commands::context::{CliVerb, ExecutionContext, VerbGroup, run_cli_verb};
use crate::query::commands::CommandDescriptor;
use clap::{Args, Command as ClapCommand};
use std::io::{self, Write};
use std::path::Path;

/// Top-level architecture and discovery flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct ArchitectureArgs {
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
    /// Enforce architectural layering rules from file (exits 2 on violation)
    #[arg(long = "arch", value_name = "RULES_FILE")]
    pub arch: Option<String>,
    /// Emit Mermaid flowchart diagram of module call graph
    #[arg(long = "mermaid")]
    pub mermaid: bool,
    /// Multi-level hierarchical Louvain community clustering
    #[arg(long = "zoom", value_name = "DEPTH")]
    pub zoom: Option<Option<u32>>,
    /// Max depth levels for --zoom
    #[arg(long = "zoom-levels", value_name = "N")]
    pub zoom_levels: Option<u32>,
    /// Include built-in functions in external-surface
    #[arg(long = "include-builtins")]
    pub include_builtins: bool,
    /// SARIF format export for lint
    #[arg(long = "sarif")]
    pub sarif: bool,
    /// Annotate every symbol with fan-in, fan-out, complexity, and change-amplification
    #[arg(long = "metrics")]
    pub metrics: bool,
    /// Entire quality panel in one report (strict|default|lenient)
    #[arg(long = "quality-panel", num_args = 0..=1, default_missing_value = "default", value_name = "PRESET")]
    pub quality_panel: Option<Option<String>>,
    /// Show one struct's memory layout with computed field offsets and padding
    #[arg(long = "layout", value_name = "STRUCT")]
    pub layout: Option<String>,
    /// Cache-conscious field affinity analysis
    #[arg(long = "field-affinity", num_args = 0..=1, default_missing_value = "", value_name = "STRUCT")]
    pub field_affinity: Option<Option<String>>,
    /// Local-reasoning lens measuring share of context outside a unit's file
    #[arg(long = "context-ratio")]
    pub context_ratio: bool,
    /// Four orthogonal evidence families joined, ranked by distinct families fired
    #[arg(long = "ensemble")]
    pub ensemble: bool,
    /// Comment-to-identifier coherence check
    #[arg(long = "comment-coherence")]
    pub comment_coherence: bool,
}

impl VerbGroup for ArchitectureArgs {
    fn is_group_active(&self) -> bool {
        self.community.is_some()
            || self.around.is_some()
            || self.arch.is_some()
            || self.zoom.is_some()
            || self.quality_panel.is_some()
            || self.layout.is_some()
            || self.field_affinity.is_some()
            || self.dead_code
            || self.communities
            || self.deps
            || self.external_surface
            || self.mermaid
            || self.metrics
            || self.context_ratio
            || self.ensemble
            || self.comment_coherence
    }
}

/// Decoded architecture flags; defaults when the command line was not parsed with this plugin.
#[must_use]
pub fn args(cli: &Cli) -> ArchitectureArgs {
    cli.plugins.decode::<ArchitectureArgs>().unwrap_or_default()
}

/// Reject `--zoom-levels` without `--zoom`.
pub fn validate_modifiers(cli: &Cli) -> io::Result<()> {
    let a = args(cli);
    if a.zoom_levels.is_some() && a.zoom.is_none() {
        return Err(io::Error::other(
            "ripwire-rs: --zoom-levels=N modifies --zoom; pass it too (e.g. --zoom --zoom-levels=0 prints every level)",
        ));
    }
    Ok(())
}

pub struct Architecture;

pub static ARCHITECTURE: Architecture = Architecture;

impl Command for Architecture {
    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        ArchitectureArgs::augment_args(cmd)
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let args = cli.plugins.decode::<ArchitectureArgs>()?;
        run_cli_verb(&args, cli, root)
    }

    fn tools(&self) -> &'static [&'static CommandDescriptor] {
        tools::TOOLS
    }
}

impl CliVerb for ArchitectureArgs {
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
                &DeadCodeContext {
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

