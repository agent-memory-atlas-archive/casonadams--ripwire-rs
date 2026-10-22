//! `--note-add NOTE`: pin a gotcha note to `.ripwire-rs_notes`.
use super::{Plugin, Run};
use clap::Args;
use std::io::{self, Write};
use std::path::Path;

#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct NoteAddArgs {
    /// Pin gotcha note to .ripwire-rs_notes (e.g. --note-add="SYM: text")
    #[arg(long = "note-add", value_name = "NOTE")]
    pub note_add: Option<String>,
}

pub static NOTE_ADD: Plugin<NoteAddArgs> = Plugin {
    name: "note-add",
    options: &[],
    selected: |args| args.note_add.is_some(),
    run,
    check: None,
    tools: &[],
};

fn run(run: &Run<'_>, args: &NoteAddArgs) -> io::Result<()> {
    let note = args.note_add.as_deref().unwrap_or_default();
    run.report(|ctx, out| execute_note_append(ctx.root, note, out))
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
