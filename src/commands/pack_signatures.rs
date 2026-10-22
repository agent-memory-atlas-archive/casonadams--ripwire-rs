//! `--pack-signatures`: the whole-repo signatures-only inventory wrapped in a `ripwire-rs.pack-signatures/v1` context.
use super::common::retrieval::report_query;
use super::{Plugin, Run};
use crate::query::commands::QueryContext;
use clap::Args;
use std::io::{self, Write};

/// `--pack-signatures`.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct PackSignaturesArgs {
    #[arg(long = "pack-signatures")]
    pub pack_signatures: bool,
}

pub fn execute<W: Write>(query: &QueryContext<'_>, out: &mut W) -> io::Result<()> {
    writeln!(out, "<ctx schema=\"ripwire-rs.pack-signatures/v1\">")?;
    let opts = crate::serialize::SerializeOptions {
        root: query.root.to_string(),
        ..Default::default()
    };
    crate::serialize::serialize_xml(query.ingest, query.graph, query.pr, query.interner, out, &opts)?;
    writeln!(out, "</ctx>")
}

fn run(run: &Run<'_>, _args: &PackSignaturesArgs) -> io::Result<()> {
    report_query(run, |query, out| execute(query, out))
}

pub static PACK_SIGNATURES: Plugin<PackSignaturesArgs> = Plugin {
    name: "pack-signatures",
    options: &[],
    selected: |args| args.pack_signatures,
    run,
    check: None,
    tools: &[],
};
