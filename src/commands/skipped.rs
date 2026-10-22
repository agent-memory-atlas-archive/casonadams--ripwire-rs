//! `--skipped`: list files skipped due to size limits, unsupported extensions, or unindexed extensions.
use super::SharedOption;
use super::plugin::report_command;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::domain::IngestResult;
use crate::serialize::{PageRequest, Paged, XmlEscapeExt, XmlWriter, row_bytes};
use std::io::{self, Write};

report_command! {
    name: "skipped",
    args: SkippedArgs,
    static: SKIPPED,
    options: [SharedOption::Paged],
    run: report(render),
    /// List files skipped due to size limits
    #[arg(long = "skipped")]
    skipped: bool,
}

fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _args: &SkippedArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let opts = crate::crawler::CrawlerOptions::default();
    let census = crate::crawler::scan_skipped_census(ctx.root, &opts);
    render_skipped_census_xml(
        ctx.root,
        &census,
        &opts,
        ctx.ingest,
        ctx.cli.page_request(),
        out,
    )
}

pub fn render_skipped_census_xml<W: Write>(
    root: &str,
    census: &crate::crawler::SkippedCensus,
    opts: &crate::crawler::CrawlerOptions,
    ingest: &IngestResult,
    page: PageRequest,
    out: &mut W,
) -> io::Result<()> {
    let root_esc = root.xml_escaped();
    let oversize_cnt = census.oversize.len();
    let excluded_cnt = census.excluded.len();
    let unsupported_cnt = census.unsupported.len();
    let total_skipped = oversize_cnt + excluded_cnt + unsupported_cnt;
    writeln!(
        out,
        "<ctx schema=\"ripwire-rs.skipped/v1\" root=\"{root_esc}\"><skipped schema=\"ripwire-rs.skipped/v1\" root=\"{root_esc}\" indexed=\"{}\" oversize=\"{oversize_cnt}\" excluded=\"{excluded_cnt}\" unsupported_ext=\"{unsupported_cnt}\" max_file_size=\"{}\" json_ceiling=\"{}\" yaml_ceiling=\"{}\" count=\"{total_skipped}\">",
        ingest.files.len(),
        opts.max_file_size,
        crate::crawler::DEFAULT_JSON_CEILING,
        crate::crawler::DEFAULT_YAML_CEILING
    )?;
    let oversize = Paged::new(&census.oversize, page, |(p, bytes, ext)| {
        row_bytes(|w| write_skipped_file_row(w, p, "oversize", bytes, ext))
    });
    for (p, bytes, ext) in oversize.rows() {
        write_skipped_file_row(out, p, "oversize", bytes, ext)?;
    }
    let unsupported = Paged::new(&census.unsupported, page, |(p, bytes, ext)| {
        row_bytes(|w| write_skipped_file_row(w, p, "unsupported-ext", bytes, ext))
    });
    for (p, bytes, ext) in unsupported.rows() {
        write_skipped_file_row(out, p, "unsupported-ext", bytes, ext)?;
    }
    let unindexed = Paged::new(&census.unindexed_exts, page, |(ext, files)| {
        row_bytes(|w| write_unindexed_row(w, ext, files))
    });
    for (ext, files) in unindexed.rows() {
        write_unindexed_row(out, ext, files)?;
    }
    let mut lang_files: std::collections::BTreeMap<&'static str, (usize, usize)> =
        std::collections::BTreeMap::new();
    for f in &ingest.files {
        lang_files.entry(f.lang.tag()).or_default().0 += 1;
    }
    for s in &ingest.symbols {
        lang_files.entry(s.lang.tag()).or_default().1 += 1;
    }
    for (tag, (f_cnt, s_cnt)) in lang_files {
        writeln!(
            out,
            "  <lang n=\"{tag}\" files=\"{f_cnt}\" symbols=\"{s_cnt}\"/>"
        )?;
    }
    writeln!(out, "</skipped></ctx>")
}

fn write_skipped_file_row<W: Write + ?Sized>(
    out: &mut W,
    p: &str,
    why: &str,
    bytes: impl std::fmt::Display,
    ext: &str,
) -> io::Result<()> {
    writeln!(
        out,
        "  <f p=\"{}\" why=\"{why}\" bytes=\"{bytes}\" ext=\"{}\"/>",
        p.xml_escaped(),
        ext.xml_escaped()
    )
}

fn write_unindexed_row<W: Write + ?Sized>(
    out: &mut W,
    ext: &str,
    files: impl std::fmt::Display,
) -> io::Result<()> {
    writeln!(out, "  <e x=\"{}\" files=\"{files}\"/>", ext.xml_escaped())
}
