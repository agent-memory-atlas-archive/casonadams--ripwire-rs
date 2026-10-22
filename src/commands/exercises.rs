//! `--exercises TESTFILE`: shows what a test file actually covers, the non-test symbols it reaches.
use super::common::safety_queries::execute_exercises_query;
use super::plugin::report_command;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "exercises",
    args: ExercisesArgs,
    static: EXERCISES,
    run: report(render),
    /// Show what a test file actually covers: non-test symbols it reaches
    #[arg(long, value_name = "TESTFILE")]
    exercises: Option<String>,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    args: &ExercisesArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let testfile = args.exercises.as_deref().unwrap_or_default();
    execute_exercises_query(
        ctx.root,
        testfile,
        ctx.ingest,
        ctx.graph,
        ctx.interner,
        out,
    )
}
