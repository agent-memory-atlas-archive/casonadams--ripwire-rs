//! `--layout STRUCT`: one struct's memory layout with computed field offsets and padding.
use super::{Plugin, Run};
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct LayoutArgs {
    /// Show one struct's memory layout with computed field offsets and padding
    #[arg(long = "layout", value_name = "STRUCT")]
    pub layout: Option<String>,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    args: &LayoutArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    use crate::domain::CorpusView;
    let sym = args.layout.as_deref().unwrap_or_default();
    let has_c_agg = ctx
        .ingest
        .symbols
        .iter()
        .any(|s| ctx.sym_name(s) == sym && s.lang.semantics().supports_c_layout());
    if !has_c_agg {
        return Err(io::Error::other(format!(
            "ripwire-rs: --layout: '{sym}' is indexed but has no C-family aggregate body — this verb models C/C++/ObjC byte layout only"
        )));
    }
    let rep = crate::metrics::compute_layout_report(sym, ctx.ingest, ctx.interner, ctx.root);
    ctx.write_report(&rep, out).map(|_| ())
}

fn run(run: &Run<'_>, args: &LayoutArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub static LAYOUT: Plugin<LayoutArgs> = Plugin {
    name: "layout",
    options: &[],
    selected: |args| args.layout.is_some(),
    run,
    check: None,
    tools: &[],
};
