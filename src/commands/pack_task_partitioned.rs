//! `--pack-task TASK --partition N`: the partitioned task bundle; it takes precedence over the plain `--pack-task`.
//! It declares no flags of its own: `--pack-task` belongs to [`super::pack_task`] and `--partition` to safety.
use super::common::retrieval::report_query;
use super::pack_task::PackTaskArgs;
use super::{Command, Run};
use crate::cli::Cli;
use std::io;

pub struct PackTaskPartitioned;
pub static PACK_TASK_PARTITIONED: PackTaskPartitioned = PackTaskPartitioned;

impl Command for PackTaskPartitioned {
    fn run(&self, cli: &Cli, root: &str) -> Option<io::Result<()>> {
        let task = cli.plugins.decode::<PackTaskArgs>()?.pack_task?;
        let partition = crate::commands::safety::args(cli).partition?;
        Some(report_query(&Run::new(cli, root, &[]), |query, out| {
            crate::navigate::execute_pack_task_partitioned(
                &task,
                query.root,
                partition.unwrap_or(2),
                query.ingest,
                query.graph,
                query.interner,
                out,
            )
        }))
    }
}
