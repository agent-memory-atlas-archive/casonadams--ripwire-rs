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

    /// The window request. `--limit/--offset` apply only when the command declared [`SharedOption::Paged`];
    /// the output budget that default caps are computed from always applies.
    #[must_use]
    pub fn page(&self) -> PageRequest {
        let page = self.cli.page_request();
        if self.options.contains(&SharedOption::Paged) {
            page
        } else {
            PageRequest::default().with_budget(page.budget)
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

/// Whether a flag's parsed value selects its command: a switch when set, an option when given.
pub trait Selected {
    fn is_selected(&self) -> bool;
}

impl Selected for bool {
    fn is_selected(&self) -> bool {
        *self
    }
}

impl<T> Selected for Option<T> {
    fn is_selected(&self) -> bool {
        self.is_some()
    }
}

/// Declares a single-flag report command: its derive `Args` (the one flag, with the caller's doc and `#[arg]`),
/// the `selected` predicate of that flag, the `run` wrapper and the `Plugin` static.
///
/// `run` picks how the command executes: `report(render)` calls `render(ctx, args, out)` through
/// [`Run::report`]; `query(execute)` calls `execute(query, value, out)` through `report_query` and `nav(mode)`
/// calls `run_nav(run, value, mode)`. `query` and `nav` read the `Option<String>` flag's value.
macro_rules! report_command {
    (
        name: $name:literal,
        args: $args:ident,
        static: $static:ident,
        $(options: [$($option:expr),* $(,)?],)?
        $(tools: [$($tool:expr),* $(,)?],)?
        run: $mode:ident($target:expr),
        $(#[$attr:meta])*
        $field:ident: $ty:ty $(,)?
    ) => {
        #[derive(clap::Args, Debug, Clone, Default, PartialEq)]
        pub struct $args {
            $(#[$attr])*
            pub $field: $ty,
        }

        fn run(run: &$crate::commands::Run<'_>, args: &$args) -> std::io::Result<()> {
            report_command!(@run $mode $target, run, args, $field)
        }

        pub static $static: $crate::commands::Plugin<$args> = $crate::commands::Plugin {
            name: $name,
            options: &[$($($option),*)?],
            selected: |args| $crate::commands::plugin::Selected::is_selected(&args.$field),
            run,
            check: None,
            tools: &[$($($tool),*)?],
        };
    };
    (@run report $render:expr, $run:ident, $args:ident, $field:ident) => {
        $run.report(|ctx, out| $render(ctx, $args, out))
    };
    (@run query $execute:expr, $run:ident, $args:ident, $field:ident) => {
        $crate::commands::common::retrieval::report_query($run, |query, out| {
            $execute(query, $args.$field.as_deref().unwrap_or_default(), out)
        })
    };
    (@run nav $mode:expr, $run:ident, $args:ident, $field:ident) => {
        $crate::commands::nav::run_nav($run, $args.$field.as_deref().unwrap_or_default(), $mode)
    };
}

pub(crate) use report_command;
