//! Workspace audit group: `--whereis`, `--owners`, `--doc-drift`, `--flags`, `--stray-content`, `--mentions`,
//! `--doctor`, `--handoff`, `--notes`, `--note-add`, `--flip`, `--plan-lint`, `--skipped`, and the symbol
//! edits `--replace-symbol-body`, `--insert-before-symbol`, `--insert-after-symbol`.

mod execute;
pub mod doctor;
mod tools;

pub use execute::*;

use super::Command;
use crate::cli::Cli;
use crate::commands::context::{CliVerb, ExecutionContext, Paginated, VerbGroup, run_cli_verb};
use crate::query::commands::CommandDescriptor;
use clap::{Args, Command as ClapCommand};
use std::io::{self, Write};
use std::path::Path;

/// Top-level workspace & audit flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct WorkspaceArgs {
    /// Find which branches or trees define a symbol
    #[arg(long, value_name = "SYM")]
    pub whereis: Option<String>,
    /// Emit verbatim matched lines in --whereis without length truncation
    #[arg(long = "whereis-full-text")]
    pub whereis_full_text: bool,
    /// Select --whereis listing filter (defs|refs|all)
    #[arg(long = "whereis-listing", value_name = "MODE")]
    pub whereis_listing: Option<String>,
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
    /// Environment and dependency health diagnostics
    #[arg(long)]
    pub doctor: bool,
    /// Continuation packet for interrupted agent sessions
    #[arg(long)]
    pub handoff: bool,
    /// View pinned session gotchas and invariants (.ripwire-rs_notes)
    #[arg(long)]
    pub notes: bool,
    /// Pin gotcha note to .ripwire-rs_notes (e.g. --note-add="SYM: text")
    #[arg(long = "note-add", value_name = "NOTE")]
    pub note_add: Option<String>,
    /// Scan blast radius of toggling a feature flag
    #[arg(long = "flip", value_name = "GATE")]
    pub flip: Option<String>,
    /// Gateability check for doc-drift
    #[arg(long = "gateability")]
    pub gateability: bool,
    /// Check ABI byte layout across branches in stray-content
    #[arg(long = "abi")]
    pub abi: bool,
    /// Generate landing plan in stray-content
    #[arg(long = "plan")]
    pub plan: bool,
    /// Lint structure of a PLAN file
    #[arg(long = "plan-lint", value_name = "FILE")]
    pub plan_lint: Option<String>,
    /// List files skipped due to size limits
    #[arg(long = "skipped")]
    pub skipped: bool,
    /// Replace symbol definition body with text
    #[arg(long = "replace-symbol-body", value_name = "SYM:BODY")]
    pub replace_symbol_body: Option<String>,
    /// Insert text immediately before a symbol definition
    #[arg(long = "insert-before-symbol", value_name = "SYM:TEXT")]
    pub insert_before_symbol: Option<String>,
    /// Insert text immediately after a symbol definition
    #[arg(long = "insert-after-symbol", value_name = "SYM:TEXT")]
    pub insert_after_symbol: Option<String>,
}

impl WorkspaceArgs {
    #[must_use]
    pub fn active_edit_op(&self) -> Option<(&str, crate::query::edit::EditOp)> {
        self.replace_symbol_body
            .as_deref()
            .map(|s| (s, crate::query::edit::EditOp::ReplaceBody))
            .or_else(|| {
                self.insert_before_symbol
                    .as_deref()
                    .map(|s| (s, crate::query::edit::EditOp::InsertBefore))
            })
            .or_else(|| {
                self.insert_after_symbol
                    .as_deref()
                    .map(|s| (s, crate::query::edit::EditOp::InsertAfter))
            })
    }
}

impl VerbGroup for WorkspaceArgs {
    fn is_group_active(&self) -> bool {
        self.whereis.is_some()
            || self.mentions.is_some()
            || self.note_add.is_some()
            || self.flip.is_some()
            || self.plan_lint.is_some()
            || self.owners
            || self.doc_drift
            || self.flags
            || self.stray_content
            || self.doctor
            || self.handoff
            || self.notes
            || self.skipped
            || self.active_edit_op().is_some()
    }
}

impl CliVerb for WorkspaceArgs {
    fn execute_verb<W: Write>(
        &self,
        ctx: &ExecutionContext<'_>,
        out: &mut crate::serialize::XmlWriter<'_, W>,
    ) -> io::Result<()> {
        if self.owners {
            return execute_workspace_owners(ctx, out);
        }
        if self.stray_content {
            let val = crate::query::doc_tools::call_stray_content(ctx.root);
            return crate::serialize::serialize_stray_content_xml(&val, out);
        }
        if let Some((spec, op)) = self.active_edit_op() {
            let safety = crate::commands::safety::args(ctx.cli);
            execute_edit_op_with_options(
                EditCliOptions {
                    root: ctx.root,
                    spec,
                    op,
                    payload_override: safety.edit_payload.as_deref(),
                    dry_run: safety.dry_run,
                },
                ctx.ingest,
                ctx.interner,
                out,
            )
        } else {
            let sources = crate::query::FileSources {
                root: Path::new(ctx.root),
            };
            let notes_content = self.notes.then(|| {
                std::fs::read_to_string(Path::new(ctx.root).join(".ripwire-rs_notes"))
                    .unwrap_or_default()
            });
            let plan_content = self
                .plan_lint
                .as_deref()
                .and_then(|path| std::fs::read_to_string(path).ok());
            let workspace = WorkspaceContext {
                root: ctx.root,
                ingest: ctx.ingest,
                interner: ctx.interner,
                sources: &sources,
                notes_content: notes_content.as_deref(),
                plan_content: plan_content.as_deref(),
            };
            execute_workspace_doc_query_paged(
                self,
                &workspace,
                ctx.cli.modifiers.with_history,
                ctx.cli,
                out,
            )
        }
    }
}

fn execute_workspace_owners<W: Write>(ctx: &ExecutionContext<'_>, out: &mut W) -> io::Result<()> {
    let cutoff = crate::git::history::resolve_cutoff_timestamp(None, 12);
    let commits = crate::git::history::walk_repository_history_scoped(ctx.root, cutoff).ok();
    let at_stamp = crate::git::history::compute_git_at_stamp(ctx.root);
    let known_files: std::collections::HashSet<String> = ctx
        .ingest
        .files
        .iter()
        .map(|f| ctx.interner.resolve(&f.path).to_string())
        .collect();
    let mut val = crate::query::doc_tools::call_owners(
        commits.as_deref(),
        None,
        at_stamp.as_deref(),
        Some(&known_files),
    );
    val["root"] = serde_json::Value::String(ctx.root.to_string());
    ctx.apply_pagination_to_json(&mut val);
    if crate::commands::for_task::detail(ctx.cli).is_some_and(|d| d > 0) {
        val["detail"] = serde_json::Value::Bool(true);
    }
    crate::serialize::serialize_owners_xml(&val, out)
}

pub struct Workspace;

pub static WORKSPACE: Workspace = Workspace;

impl Command for Workspace {
    fn augment(&self, cmd: ClapCommand) -> ClapCommand {
        WorkspaceArgs::augment_args(cmd)
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let args = cli.plugins.decode::<WorkspaceArgs>()?;
        run_cli_verb(&args, cli, root)
    }

    fn tools(&self) -> &'static [&'static CommandDescriptor] {
        tools::TOOLS
    }
}
