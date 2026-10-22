//! `--notes`: view pinned session gotchas and invariants (`.ripwire-rs_notes`).
use super::plugin::report_command;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use std::io::{self, Write};
use std::path::Path;

report_command! {
    name: "notes",
    args: NotesArgs,
    static: NOTES,
    run: report(render),
    /// View pinned session gotchas and invariants (.ripwire-rs_notes)
    #[arg(long)]
    notes: bool,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    _: &NotesArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let content = std::fs::read_to_string(Path::new(ctx.root).join(".ripwire-rs_notes"))
        .unwrap_or_default();
    render_notes_xml(&content, out)
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

