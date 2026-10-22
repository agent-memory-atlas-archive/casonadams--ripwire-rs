//! `--flip GATE`: scan the blast radius of toggling a feature flag.
use super::{Plugin, Run};
use crate::query::FileSources;
use crate::query::doc_tools;
use crate::serialize::XmlEscapeExt;
use clap::Args;
use std::io::{self, Write};
use std::path::Path;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct FlipArgs {
    /// Scan blast radius of toggling a feature flag
    #[arg(long = "flip", value_name = "GATE")]
    pub flip: Option<String>,
}

pub static FLIP: Plugin<FlipArgs> = Plugin {
    name: "flip",
    options: &[],
    selected: |args| args.flip.is_some(),
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, args: &FlipArgs) -> io::Result<()> {
    let gate = args.flip.as_deref().unwrap_or_default();
    run.report(|ctx, out| {
        let sources = FileSources {
            root: Path::new(ctx.root),
        };
        let val = doc_tools::flags_from_sources(ctx.root, &sources, ctx.ingest, ctx.interner);
        render_flip_xml(gate, ctx.root, &val, out)
    })
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
