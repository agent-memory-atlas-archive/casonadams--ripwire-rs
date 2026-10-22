//! `--test-gate [FILES]`: names tests to run and the untested blast radius; exits 4 when non-empty.
use super::common::safety::report_exit;
use super::common::safety_queries::execute_test_gate_query_paged;
use super::{Plugin, Run, SharedOption};
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TestGateArgs {
    /// Before a PR: name tests to run and untested blast radius (exit 4 if non-empty)
    #[arg(long = "test-gate", value_name = "FILES")]
    pub test_gate: Option<Option<String>>,
}

fn run(run: &Run<'_>, args: &TestGateArgs) -> io::Result<()> {
    let files = args.test_gate.clone().flatten();
    report_exit(run, |ctx, out| {
        let failed = execute_test_gate_query_paged(
            ctx.root,
            files.as_deref(),
            run.page(),
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            out,
        )?;
        Ok(failed.then_some(4))
    })
}

pub static TEST_GATE: Plugin<TestGateArgs> = Plugin {
    name: "test-gate",
    options: &[SharedOption::Paged],
    selected: |args| args.test_gate.is_some(),
    run,
    check: None,
    tools: &[],
};
