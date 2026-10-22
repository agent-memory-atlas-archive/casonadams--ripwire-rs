//! `--comment-coherence`: comment-to-identifier coherence check.
use super::{Plugin, Run, SharedOption};
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use clap::Args;
use std::io;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct CommentCoherenceArgs {
    /// Comment-to-identifier coherence check
    #[arg(long = "comment-coherence")]
    pub comment_coherence: bool,
}

pub static COMMENT_COHERENCE: Plugin<CommentCoherenceArgs> = Plugin {
    name: "comment-coherence",
    options: &[SharedOption::Paged],
    selected: |a| a.comment_coherence,
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, args: &CommentCoherenceArgs) -> io::Result<()> {
    run.report(|ctx, out| render(ctx, args, out))
}

pub fn render(
    ctx: &ExecutionContext<'_>,
    _args: &CommentCoherenceArgs,
    out: &mut XmlWriter<'_, impl std::io::Write>,
) -> io::Result<()> {
    let rep = crate::metrics::analysis_lenses::compute_comment_coherence_in_root(
        ctx.root,
        ctx.ingest,
        ctx.interner,
        ctx.cli.page_request(),
    );
    ctx.write_report(&rep, out)?;
    Ok(())
}
