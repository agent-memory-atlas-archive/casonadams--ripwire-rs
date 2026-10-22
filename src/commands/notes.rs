//! `--notes`: view pinned session gotchas and invariants (`.ripwire-rs_notes`).
use super::{Plugin, Run};
use clap::Args;
use std::io::{self, Write};
use std::path::Path;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct NotesArgs {
    /// View pinned session gotchas and invariants (.ripwire-rs_notes)
    #[arg(long)]
    pub notes: bool,
}

pub static NOTES: Plugin<NotesArgs> = Plugin {
    name: "notes",
    options: &[],
    selected: |args| args.notes,
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, _: &NotesArgs) -> io::Result<()> {
    run.report(|ctx, out| {
        let content = std::fs::read_to_string(Path::new(ctx.root).join(".ripwire-rs_notes"))
            .unwrap_or_default();
        render_notes_xml(&content, out)
    })
}

pub fn render_notes_xml<W: Write>(content: &str, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    NotesReport { content }.render_envelope(out)
}

struct NotesReport<'a> {
    content: &'a str,
}

impl crate::serialize::XmlReport for NotesReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "ctx"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire-rs.notes/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        use crate::serialize::XmlEscapeExt;
        write!(out, "<notes>")?;
        for line in self.content.lines().filter(|l| !l.trim().is_empty()) {
            write!(out, "<note text=\"{}\"/>", line.xml_escaped())?;
        }
        writeln!(out, "</notes>")
    }
}

