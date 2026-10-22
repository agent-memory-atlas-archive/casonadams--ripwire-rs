//! `--pack-task TASK`: a single-call orientation bundle for a task under the token budget.
//! The partitioned variant lives in [`super::pack_task_partitioned`].
use super::plugin::report_command;
use crate::query::commands::QueryContext;
use std::io::{self, Write};

pub fn execute<W: Write>(query: &QueryContext<'_>, task: &str, out: &mut W) -> io::Result<()> {
    crate::navigate::execute_pack_task(task, query, out)
}

report_command! {
    name: "pack-task",
    args: PackTaskArgs,
    static: PACK_TASK,
    run: query(execute),
    /// Package single-call orientation task bundle under token budget
    #[arg(long = "pack-task", value_name = "TASK")]
    pack_task: Option<String>,
}
