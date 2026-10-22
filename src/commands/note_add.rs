//! `--note-add NOTE`: pin a gotcha note to `.ripwire-rs_notes`.
use super::plugin::report_command;
use crate::commands::context::ExecutionContext;
use crate::serialize::XmlWriter;
use std::io::{self, Write};
use std::path::Path;

report_command! {
    name: "note-add",
    args: NoteAddArgs,
    static: NOTE_ADD,
    run: report(render),
    /// Pin gotcha note to .ripwire-rs_notes (e.g. --note-add="SYM: text")
    #[arg(long = "note-add", value_name = "NOTE")]
    note_add: Option<String>,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    args: &NoteAddArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let note = args.note_add.as_deref().unwrap_or_default();
    execute_note_append(ctx.root, note, out)
}

fn execute_note_append<W: Write>(root: &str, note: &str, out: &mut W) -> io::Result<()> {
    let notes_path = Path::new(root).join(".ripwire-rs_notes");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&notes_path)?;
    writeln!(file, "{note}")?;
    writeln!(out, "pinned note: {note}")
}
