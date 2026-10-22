//! `--skipped`: list files skipped due to size limits, unsupported extensions, or unindexed extensions.
use super::{Plugin, Run, SharedOption};
use crate::domain::IngestResult;
use crate::serialize::{PageRequest, XmlEscapeExt};
use clap::Args;
use std::io::{self, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct SkippedArgs {
    /// List files skipped due to size limits
    #[arg(long = "skipped")]
    pub skipped: bool,
}

pub static SKIPPED: Plugin<SkippedArgs> = Plugin {
    name: "skipped",
    options: &[SharedOption::Paged],
    selected: |args| args.skipped,
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, _: &SkippedArgs) -> io::Result<()> {
    let page = run.page();
    run.report(|ctx, out| {
        let opts = crate::crawler::CrawlerOptions::default();
        let census = crate::crawler::scan_skipped_census(ctx.root, &opts);
        render_skipped_census_xml(ctx.root, &census, &opts, ctx.ingest, page, out)
    })
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
    let oversize = page.window(census.oversize.len(), 40);
    for (p, bytes, ext) in &census.oversize[oversize.begin..oversize.end] {
        writeln!(
            out,
            "  <f p=\"{}\" why=\"oversize\" bytes=\"{bytes}\" ext=\"{}\"/>",
            p.xml_escaped(),
            ext.xml_escaped()
        )?;
    }
    let unsupported = page.window(census.unsupported.len(), 40);
    for (p, bytes, ext) in &census.unsupported[unsupported.begin..unsupported.end] {
        writeln!(
            out,
            "  <f p=\"{}\" why=\"unsupported-ext\" bytes=\"{bytes}\" ext=\"{}\"/>",
            p.xml_escaped(),
            ext.xml_escaped()
        )?;
    }
    let unindexed = page.window(census.unindexed_exts.len(), 30);
    for (ext, files) in &census.unindexed_exts[unindexed.begin..unindexed.end] {
        writeln!(out, "  <e x=\"{}\" files=\"{files}\"/>", ext.xml_escaped())?;
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
