//! `--safe-delete SYM`: safe-delete assessment joining callers, blast radius, tests, and history.
use super::common::safety::context_of;
use super::common::safety_queries::execute_safe_delete_query;
use super::{Plugin, Run};
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct SafeDeleteArgs {
    /// Safe delete assessment joining callers, blast radius, tests, and history
    #[arg(long = "safe-delete", value_name = "SYM")]
    pub safe_delete: Option<String>,
}

fn run(run: &Run<'_>, args: &SafeDeleteArgs) -> io::Result<()> {
    let sym = args.safe_delete.as_deref().unwrap_or_default();
    run.report(|ctx, out| {
        execute_safe_delete_query(&context_of(ctx), sym, run.page(), out)
    })
}

pub static SAFE_DELETE: Plugin<SafeDeleteArgs> = Plugin {
    name: "safe-delete",
    options: &[],
    selected: |args| args.safe_delete.is_some(),
    run,
    check: None,
    tools: &[],
};
