//! `--flip GATE`: scan the blast radius of toggling a feature flag.
use super::plugin::report_command;
use crate::commands::context::ExecutionContext;
use crate::query::FileSources;
use crate::query::doc_tools;
use crate::serialize::{XmlEscapeExt, XmlWriter};
use std::io::{self, Write};
use std::path::Path;

report_command! {
    name: "flip",
    args: FlipArgs,
    static: FLIP,
    run: report(render),
    /// Scan blast radius of toggling a feature flag
    #[arg(long = "flip", value_name = "GATE")]
    flip: Option<String>,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    args: &FlipArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let gate = args.flip.as_deref().unwrap_or_default();
    let sources = FileSources {
        root: Path::new(ctx.root),
    };
    let val = doc_tools::flags_from_sources(ctx.root, &sources, ctx.ingest, ctx.interner);
    render_flip_xml(gate, ctx.root, &val, out)
}

pub fn render_flip_xml<W: Write>(
    gate: &str,
    root: &str,
    flags_val: &serde_json::Value,
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<flip schema=\"ripwire-rs.flip/v1\" gate=\"{}\" root=\"{}\">",
        gate.xml_escaped(),
        root.xml_escaped()
    )?;
    if let Some(arr) = flags_val["flags"].as_array() {
        for f in arr {
            let name = f["name"].as_str().unwrap_or("");
            if name.contains(gate) {
                let p = f["path"].as_str().unwrap_or("").xml_escaped();
                let l = f["line"].as_u64().unwrap_or(1);
                writeln!(
                    out,
                    "  <site p=\"{p}\" l=\"{l}\" gate=\"{}\"/>",
                    name.xml_escaped()
                )?;
            }
        }
    }
    writeln!(out, "</flip>")
}
