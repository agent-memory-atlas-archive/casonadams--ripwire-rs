//! Shared adapters for the safety commands: context conversion and exit-code reporting.

use crate::commands::context::ExecutionContext;
use crate::commands::plugin::Run;
use crate::query::commands::QueryContext;
use crate::query::safety::SafetyContext;
use crate::serialize::XmlWriter;
use std::io::{self, StdoutLock};

/// Builds the safety context from an MCP query state.
pub fn safety_context<'a>(state: &QueryContext<'a>) -> SafetyContext<'a> {
    SafetyContext {
        root: state.root,
        ingest: state.ingest,
        graph: state.graph,
        interner: state.interner,
    }
}

/// Builds the safety context from a CLI execution context.
pub fn context_of<'a>(ctx: &ExecutionContext<'a>) -> SafetyContext<'a> {
    SafetyContext {
        root: ctx.root,
        ingest: ctx.ingest,
        graph: ctx.graph,
        interner: ctx.interner,
    }
}

/// Runs a report body, then exits with its code when non-zero (after the report is finished).
pub fn report_exit(
    run: &Run<'_>,
    body: impl FnOnce(
        &ExecutionContext<'_>,
        &mut XmlWriter<'_, StdoutLock<'static>>,
    ) -> io::Result<Option<i32>>,
) -> io::Result<()> {
    let mut code = None;
    run.report(|ctx, out| {
        code = body(ctx, out)?;
        Ok(())
    })?;
    match code {
        Some(c) if c > 0 => std::process::exit(c),
        _ => Ok(()),
    }
}
