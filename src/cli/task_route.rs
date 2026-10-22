use crate::domain::{IngestResult, Interner};
use std::io::{self, Write};

pub fn write_help_task_xml<W: Write>(
    task: &str,
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    crate::query::task_route::write_help_task_with_facts(
        task,
        root,
        ingest,
        interner,
        gix::open(root).is_ok(),
        !crate::cli::safety::get_working_tree_changed_files(root).is_empty(),
        out,
    )
}
