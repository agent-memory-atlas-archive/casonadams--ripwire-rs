//! `--situ [FILES]`: blast radius, tests, and co-change for a change (default: git diff); MCP `situational_awareness`.
use super::common::mcp::mcp_tool;
use super::common::safety_queries::execute_situ_query;
use super::{Plugin, Run};
use crate::query::commands::{render_xml, resolve_state_commits};
use crate::serialize::JsonValueExt;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct SituArgs {
    /// Situational awareness for a change: blast radius + tests + co-change (default: git diff)
    #[arg(long, value_name = "FILES")]
    pub situ: Option<Option<String>>,
}

fn run(run: &Run<'_>, args: &SituArgs) -> io::Result<()> {
    let files = args.situ.clone().flatten();
    run.report(|ctx, out| {
        execute_situ_query(
            ctx.root,
            files.as_deref(),
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            out,
        )
    })
}

pub static SITU: Plugin<SituArgs> = Plugin {
    name: "situ",
    options: &[],
    selected: |args| args.situ.is_some(),
    run,
    check: None,
    tools: &[mcp_tool!(
        "situational_awareness",
        "The 5 things to know about a diff: blast_radius, tests_to_run, forgotten co-change files, hotspots",
        &[("files", "string[]", "Changed file paths")],
        &["files"],
        |s, a| {
            let commits = resolve_state_commits(s, 18);
            let files = a.str_vec("files");
            let report = crate::query::safety::SituationalReport {
                root: s.root,
                files: &files,
                commits: Some(&commits),
                context: crate::query::safety::SafetyContext {
                    root: s.root,
                    ingest: s.ingest,
                    graph: s.graph,
                    interner: s.interner,
                },
            };
            use crate::serialize::XmlReport;
            render_xml(|out| report.render_envelope(out))
        }
    )],
};
