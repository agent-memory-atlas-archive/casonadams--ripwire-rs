//! `--flags`: dark feature flags and environment reads.
use super::common::mcp::mcp_tool;
use super::{Plugin, Run, SharedOption};
use crate::commands::context::Paginated;
use crate::query::FileSources;
use crate::query::commands::render_query_value;
use crate::query::doc_tools;
use crate::serialize::serialize_flags_xml;
use clap::Args;
use std::io;
use std::path::Path;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct FlagsArgs {
    /// Scan dark feature flags and environment reads
    #[arg(long)]
    pub flags: bool,
}

pub static FLAGS: Plugin<FlagsArgs> = Plugin {
    name: "flags",
    options: &[SharedOption::Paged],
    selected: |args| args.flags,
    run,
    check: None,
    tools: &[mcp_tool!(
        "flags",
        "WHAT IS BUILT BUT DARK: harvests ifndef header gates, CMake options, env reads",
        &[
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
        &[],
        |s, a| {
            let value =
                doc_tools::flags_from_sources(s.root, s.sources, s.ingest, s.interner);
            render_query_value(value, a, serialize_flags_xml)
        }
    )],
};

fn run(run: &Run<'_>, _: &FlagsArgs) -> io::Result<()> {
    run.report(|ctx, out| {
        let sources = FileSources {
            root: Path::new(ctx.root),
        };
        let mut val = doc_tools::flags_from_sources(ctx.root, &sources, ctx.ingest, ctx.interner);
        ctx.apply_pagination_to_json(&mut val);
        serialize_flags_xml(&val, out)
    })
}
