//! `--partition [K]`: partitions the codebase into cohesive modules; `--task GOAL` focuses it on a task.
use super::common::safety_queries::execute_partition_from_facts;
use super::{Plugin, Run};
use crate::query::safety::SafetyContext;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct PartitionArgs {
    /// Partition codebase into cohesive modules
    #[arg(long = "partition", value_name = "K")]
    pub partition: Option<Option<usize>>,
}

fn run(run: &Run<'_>, args: &PartitionArgs) -> io::Result<()> {
    let k = args.partition.flatten().unwrap_or(2);
    let task = run.cli.modifiers.task.clone();
    run.report(|ctx, out| {
        let safety = SafetyContext {
            root: ctx.root,
            ingest: ctx.ingest,
            graph: ctx.graph,
            interner: ctx.interner,
        };
        execute_partition_from_facts(&safety, k, task.as_deref(), run.page(), out)
    })
}

pub static PARTITION: Plugin<PartitionArgs> = Plugin {
    name: "partition",
    options: &[],
    selected: |args| args.partition.is_some(),
    run,
    check: None,
    tools: &[],
};
