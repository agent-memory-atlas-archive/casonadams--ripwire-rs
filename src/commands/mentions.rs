//! `--mentions SYM`: markdown docs that reference a symbol in backticks.
use super::common::mcp::mcp_tool;
use super::{Plugin, Run};
use crate::query::FileSources;
use crate::query::commands::render_xml;
use crate::query::doc_tools;
use crate::serialize::{JsonValueExt, serialize_mentions_xml};
use clap::Args;
use std::io;
use std::path::Path;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct MentionsArgs {
    /// Markdown docs referencing symbol in backticks
    #[arg(long, value_name = "SYM")]
    pub mentions: Option<String>,
}

pub static MENTIONS: Plugin<MentionsArgs> = Plugin {
    name: "mentions",
    options: &[],
    selected: |args| args.mentions.is_some(),
    run,
    check: None,
    tools: &[mcp_tool!(
        "mentions",
        "Docs that name a code symbol in a backtick",
        &[("symbol", "string", "Symbol name")],
        &["symbol"],
        |s, a| {
            let symbol = a.req_str("symbol")?;
            if crate::navigate::find_symbols_by_name(symbol, s.ingest, s.interner).is_empty() {
                return Err(format!("ripwire-rs: --mentions symbol not found: {symbol}"));
            }
            let value = doc_tools::mentions_from_sources(s.sources, s.ingest, s.interner, symbol);
            render_xml(|out| serialize_mentions_xml(&value, out))
        }
    )],
};

fn run(run: &Run<'_>, args: &MentionsArgs) -> io::Result<()> {
    let symbol = args.mentions.as_deref().unwrap_or_default();
    run.report(|ctx, out| {
        if crate::navigate::find_symbols_by_name(symbol, ctx.ingest, ctx.interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire-rs: --mentions symbol not found: {symbol}"
            )));
        }
        let sources = FileSources {
            root: Path::new(ctx.root),
        };
        let value =
            doc_tools::mentions_from_sources(&sources, ctx.ingest, ctx.interner, symbol);
        serialize_mentions_xml(&value, out)
    })
}
