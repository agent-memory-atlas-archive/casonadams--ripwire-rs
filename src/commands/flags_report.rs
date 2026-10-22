//! `--flags`: dark feature flags and environment reads.
use super::common::mcp::mcp_tool;
use super::SharedOption;
use super::plugin::report_command;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::query::FileSources;
use crate::query::commands::render_query_value;
use crate::query::doc_tools;
use crate::serialize::{XmlWriter, serialize_flags_xml};
use std::io::{self, Write};
use std::path::Path;

report_command! {
    name: "flags",
    args: FlagsArgs,
    static: FLAGS,
    options: [SharedOption::Paged],
    tools: [mcp_tool!(
        "flags",
        "WHAT IS BUILT BUT DARK: harvests ifndef header gates, CMake options, env reads",
        &[],
        &[],
        |s, a| {
            let value =
                doc_tools::flags_from_sources(s.root, s.sources, s.ingest, s.interner);
            render_query_value(value, a, serialize_flags_xml)
        }
    )],
    run: report(render),
    /// Scan dark feature flags and environment reads
    #[arg(long)]
    flags: bool,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _: &FlagsArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let sources = FileSources {
        root: Path::new(ctx.root),
    };
    let mut val = doc_tools::flags_from_sources(ctx.root, &sources, ctx.ingest, ctx.interner);
    ctx.apply_pagination_to_json(&mut val);
    serialize_flags_xml(&val, out)
}
