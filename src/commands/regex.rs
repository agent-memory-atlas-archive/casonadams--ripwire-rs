//! `--regex PAT`: regular-expression search over the corpus, sharing the `--grep` modifiers.
use super::common::grep::execute;
use super::{Plugin, Run, SharedOption};
use clap::Args;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct RegexArgs {
    /// Regular expression search
    #[arg(long = "regex", value_name = "PAT")]
    pub regex: Option<String>,
}

fn run(run: &Run<'_>, args: &RegexArgs) -> std::io::Result<()> {
    let pat = args.regex.as_deref().unwrap_or_default();
    run.report(|ctx, out| {
        execute(
            ctx,
            "regex",
            pat,
            run.page(),
            run.cli.modifiers.in_scope.as_deref(),
            &run.cli.modifiers,
            out,
        )
    })
}

pub static REGEX: Plugin<RegexArgs> = Plugin {
    name: "regex",
    options: &[SharedOption::Paged],
    selected: |args| args.regex.is_some(),
    run,
    check: None,
    tools: &[],
};
