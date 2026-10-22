//! `--recall TASK`: the doc sections most relevant to a task, as plain text unless `--json` is set.
use super::common::mcp::mcp_tool;
use super::common::retrieval::token_budget;
use super::{Plugin, Run};
use crate::commands::context::{ExecutionContext, OutputDialect};
use crate::query::commands::{CommandDescriptor, render_xml};
use crate::query::{retrieval, search};
use crate::serialize::JsonValueExt;
use clap::Args;
use std::io::{self, Write};
use std::path::Path;

/// `--recall`.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct RecallArgs {
    /// Most relevant doc sections for a task, BM25-ranked under a token ceiling
    #[arg(long, value_name = "TASK")]
    pub recall: Option<String>,
}

fn render<W: Write>(ctx: &ExecutionContext<'_>, task: &str, out: &mut W) -> io::Result<()> {
    let inputs = search::collect_recall_inputs(Path::new(ctx.root));
    let val = search::call_memory_recall(&inputs, task, ctx.cli.top_k, token_budget(ctx.cli));
    crate::serialize::serialize_recall_text(task, &val, out)
}

fn run(run: &Run<'_>, args: &RecallArgs) -> io::Result<()> {
    let task = args.recall.as_deref().unwrap_or_default();
    if run.cli.is_json_out() {
        run.report(|ctx, out| render(ctx, task, out))
    } else {
        run.report_plain(|ctx, out| render(ctx, task, out))
    }
}

pub static RECALL: Plugin<RecallArgs> = Plugin {
    name: "recall",
    options: &[],
    selected: |args| args.recall.is_some(),
    run,
    check: None,
    tools: TOOLS,
};

static TOOLS: &[&CommandDescriptor] = &[mcp_tool!(
    "memory_recall",
    "Most relevant doc sections for a task, BM25-ranked under a token ceiling",
    &[
        ("task", "string", "Task description"),
        (
            "top_k",
            "integer",
            "Maximum number of documents (default 8)"
        ),
        (
            "budget_tokens",
            "integer",
            "Body token ceiling (default 8000)"
        ),
    ],
    &["task"],
    |s, a| {
        let inputs = retrieval::collect_recall_inputs(std::path::Path::new(s.root));
        let task = a.req_str("task")?;
        let val = retrieval::call_memory_recall(
            &inputs,
            task,
            a.opt_usize("top_k"),
            a.opt_usize("budget_tokens"),
        );
        render_xml(|out| crate::serialize::serialize_recall_text(task, &val, out))
    }
)];
