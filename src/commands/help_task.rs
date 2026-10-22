//! `--help-task TASK`: recommends the best ripwire verb for a natural-language task.
use super::common::retrieval::report_query;
use super::{Plugin, Run};
use crate::query::commands::QueryContext;
use clap::Args;
use std::io::{self, Write};

/// `--help-task`.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct HelpTaskArgs {
    /// Recommend the best ripwire verb for a natural-language task
    #[arg(long = "help-task", value_name = "TASK")]
    pub help_task: Option<String>,
}

pub fn execute<W: Write>(query: &QueryContext<'_>, task: &str, out: &mut W) -> io::Result<()> {
    crate::query::task_route::write_help_task_with_facts(
        task,
        query.root,
        query.ingest,
        query.interner,
        gix::open(query.root).is_ok(),
        !crate::commands::safety::get_working_tree_changed_files(query.root).is_empty(),
        out,
    )
}

fn run(run: &Run<'_>, args: &HelpTaskArgs) -> io::Result<()> {
    let task = args.help_task.as_deref().unwrap_or_default();
    report_query(run, |query, out| execute(query, task, out))
}

pub static HELP_TASK: Plugin<HelpTaskArgs> = Plugin {
    name: "help-task",
    options: &[],
    selected: |args| args.help_task.is_some(),
    run,
    check: None,
    tools: &[],
};
