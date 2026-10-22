//! `--pack-task TASK`: a single-call orientation bundle for a task under the token budget.
//! The partitioned variant lives in [`super::pack_task_partitioned`].
use super::common::retrieval::report_query;
use super::{Plugin, Run};
use crate::query::commands::QueryContext;
use clap::Args;
use std::io::{self, Write};

/// `--pack-task`.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct PackTaskArgs {
    /// Package single-call orientation task bundle under token budget
    #[arg(long = "pack-task", value_name = "TASK")]
    pub pack_task: Option<String>,
}

pub fn execute<W: Write>(query: &QueryContext<'_>, task: &str, out: &mut W) -> io::Result<()> {
    crate::navigate::execute_pack_task(task, query, out)
}

fn run(run: &Run<'_>, args: &PackTaskArgs) -> io::Result<()> {
    let task = args.pack_task.as_deref().unwrap_or_default();
    report_query(run, |query, out| execute(query, task, out))
}

pub static PACK_TASK: Plugin<PackTaskArgs> = Plugin {
    name: "pack-task",
    options: &[],
    selected: |args| args.pack_task.is_some(),
    run,
    check: None,
    tools: &[],
};
