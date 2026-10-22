//! `--comment-coherence`: comment-to-identifier coherence check.
use super::plugin::report_command;
use super::SharedOption;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::serialize::XmlWriter;
use std::io::{self, Write};

report_command! {
    name: "comment-coherence",
    args: CommentCoherenceArgs,
    static: COMMENT_COHERENCE,
    options: [SharedOption::Paged],
    run: report(render),
    /// Comment-to-identifier coherence check
    #[arg(long = "comment-coherence")]
    comment_coherence: bool,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &CommentCoherenceArgs,
    out: &mut XmlWriter<'_, W>,
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
