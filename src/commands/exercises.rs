//! `--exercises TESTFILE`: shows what a test file actually covers, the non-test symbols it reaches.
use super::common::safety_queries::execute_exercises_query;
use super::{Plugin, Run};
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct ExercisesArgs {
    /// Show what a test file actually covers: non-test symbols it reaches
    #[arg(long, value_name = "TESTFILE")]
    pub exercises: Option<String>,
}

fn run(run: &Run<'_>, args: &ExercisesArgs) -> io::Result<()> {
    let testfile = args.exercises.as_deref().unwrap_or_default();
    run.report(|ctx, out| {
        execute_exercises_query(
            ctx.root,
            testfile,
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            out,
        )
    })
}

pub static EXERCISES: Plugin<ExercisesArgs> = Plugin {
    name: "exercises",
    options: &[],
    selected: |args| args.exercises.is_some(),
    run,
    check: None,
    tools: &[],
};
