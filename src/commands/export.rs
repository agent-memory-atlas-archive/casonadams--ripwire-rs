use super::{Plugin, Run};
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use crate::serialize::escape::escape_xml_str;
use crate::serialize::graph_export::{GraphExport, GraphFormat};
use clap::Args;
use std::io::{self, BufWriter, Write};

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct ExportArgs {
    /// Export the call graph to FILE: .graphml (Gephi, yEd, Neo4j) or .json (node-link)
    #[arg(long = "export", value_name = "FILE")]
    pub export: Option<String>,
}

fn format_of(path: &str) -> io::Result<GraphFormat> {
    GraphFormat::from_path(path).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("ripwire-rs: --export {path}: use a .graphml or .json file"),
        )
    })
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    path: &str,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let format = format_of(path)?;
    let mut file = BufWriter::new(std::fs::File::create(path)?);
    let counts = GraphExport {
        ingest: ctx.ingest,
        graph: ctx.graph,
        ranks: &ctx.pr.ranks,
        interner: ctx.interner,
    }
    .write(format, &mut file)?;
    file.flush()?;
    write!(
        out,
        "<export schema=\"ripwire-rs.export/v1\" root=\"{}\" file=\"{}\" format=\"{}\" nodes=\"{}\" edges=\"{}\"/>",
        escape_xml_str(ctx.root),
        escape_xml_str(path),
        format.label(),
        counts.nodes,
        counts.edges
    )
}

fn run(run: &Run<'_>, args: &ExportArgs) -> io::Result<()> {
    let path = args.export.as_deref().unwrap_or_default();
    run.report(|ctx, out| render(ctx, path, out))
}

fn check(args: &ExportArgs) -> io::Result<()> {
    args.export.as_deref().map_or(Ok(()), |path| format_of(path).map(drop))
}

pub static EXPORT: Plugin<ExportArgs> = Plugin {
    name: "export",
    options: &[],
    selected: |args| args.export.is_some(),
    run,
    check: Some(check),
    tools: &[],
};
