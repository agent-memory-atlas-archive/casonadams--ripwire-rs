//! `--pr-context [REF]`: review bundle per changed file vs base (symbols, callers, blast radius, tests).
use super::common::safety_queries::execute_pr_context_dispatch;
use super::{Plugin, Run};
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct PrContextArgs {
    /// Review bundle per changed file vs base: symbols, callers, blast radius, tests
    #[arg(long = "pr-context", value_name = "REF")]
    pub pr_context: Option<Option<String>>,
}

fn run(run: &Run<'_>, args: &PrContextArgs) -> io::Result<()> {
    let reference = args.pr_context.clone().flatten();
    run.report(|ctx, out| {
        execute_pr_context_dispatch(
            ctx.root,
            reference.as_deref(),
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            out,
        )
    })
}

pub static PR_CONTEXT: Plugin<PrContextArgs> = Plugin {
    name: "pr-context",
    options: &[],
    selected: |args| args.pr_context.is_some(),
    run,
    check: None,
    tools: &[],
};
