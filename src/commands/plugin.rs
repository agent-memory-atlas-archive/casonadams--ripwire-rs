//! The generic single-command plugin and the [`Run`] context it hands to its handler.
//!
//! A command module declares its derive `Args` (its own flags), a handler `fn(&Run, &Args)` and the shared
//! options it honors, then builds one `Plugin` static. Shared options (`--limit`, ...) reach the handler only
//! through `Run`, so supporting a new shared option in a command is one entry in its `options` list.
use super::Command;
use crate::cli::Cli;
use crate::commands::context::{
    ExecutionContext, OutputDialect, Paginated, load_repo_analysis_for_cli,
};
use crate::query::commands::CommandDescriptor;
use crate::serialize::{PageRequest, XmlWriter};
use std::io::{self, StdoutLock};

/// A shared CLI option a command honors; a command lists only the ones it really applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedOption {
    /// `--limit` / `--offset`, delivered by [`Run::page`].
    Paged,
}

/// What a command handler runs against: the parsed CLI, the repository root and the command's options.
pub struct Run<'a> {
    pub cli: &'a Cli,
    pub root: &'a str,
    options: &'static [SharedOption],
}

impl<'a> Run<'a> {
    #[must_use]
    pub fn new(cli: &'a Cli, root: &'a str, options: &'static [SharedOption]) -> Self {
        Self { cli, root, options }
    }

    /// The `--limit` / `--offset` window; empty unless the command declared [`SharedOption::Paged`].
    #[must_use]
    pub fn page(&self) -> PageRequest {
        if self.options.contains(&SharedOption::Paged) {
            self.cli.page_request()
        } else {
            PageRequest::default()
        }
    }

    /// Load the analysis, hand it and the shared output writer (`--json`, `--pretty`, `--legend`) to `body`.
    pub fn report(
        &self,
        body: impl FnOnce(
            &ExecutionContext<'_>,
            &mut XmlWriter<'_, StdoutLock<'static>>,
        ) -> io::Result<()>,
    ) -> io::Result<()> {
        self.render(false, body)
    }

    /// Like [`Run::report`], but the writer always emits plain markdown (`--report`, `--mermaid`).
    pub fn report_plain(
        &self,
        body: impl FnOnce(
            &ExecutionContext<'_>,
            &mut XmlWriter<'_, StdoutLock<'static>>,
        ) -> io::Result<()>,
    ) -> io::Result<()> {
        self.render(true, body)
    }

    fn render(
        &self,
        plain: bool,
        body: impl FnOnce(
            &ExecutionContext<'_>,
            &mut XmlWriter<'_, StdoutLock<'static>>,
        ) -> io::Result<()>,
    ) -> io::Result<()> {
        let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(self.cli, self.root, false)?;
        let ctx = ExecutionContext {
            cli: self.cli,
            root: self.root,
            ingest: &ingest,
            graph: &graph,
            pr: &pr,
            interner: &interner,
        };
        let mut stdout_lock = io::stdout().lock();
        let mut out = self.cli.wrap_xml_writer(&mut stdout_lock, plain);
        body(&ctx, &mut out)?;
        out.finish()
    }
}

/// One CLI command: its flags (`A`), when they select it, what it runs, and an optional flag-combination check.
pub struct Plugin<A: 'static> {
    pub name: &'static str,
    pub options: &'static [SharedOption],
    pub selected: fn(&A) -> bool,
    pub run: fn(&Run<'_>, &A) -> io::Result<()>,
    pub check: Option<fn(&A) -> io::Result<()>>,
    pub tools: &'static [&'static CommandDescriptor],
}

impl<A> Command for Plugin<A>
where
    A: clap::Args + clap::FromArgMatches + 'static,
{
    fn name(&self) -> &'static str {
        self.name
    }

    fn options(&self) -> &'static [SharedOption] {
        self.options
    }

    fn augment(&self, cmd: clap::Command) -> clap::Command {
        A::augment_args(cmd)
    }

    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let args = cli.plugins.decode::<A>()?;
        (self.selected)(&args).then(|| (self.run)(&Run::new(cli, root, self.options), &args))
    }

    fn validate(&self, cli: &Cli) -> io::Result<()> {
        match (self.check, cli.plugins.decode::<A>()) {
            (Some(check), Some(args)) => check(&args),
            _ => Ok(()),
        }
    }

    fn tools(&self) -> &'static [&'static CommandDescriptor] {
        self.tools
    }
}
