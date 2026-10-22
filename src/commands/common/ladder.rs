//! Body query behind `--expand` and `--outline`.
use crate::domain::{IngestResult, Interner};
use std::io::{self, Write};
use std::path::Path;

pub fn execute_expand_or_outline_compressed<W: Write>(
    root: &str,
    syms_spec: &str,
    is_outline: bool,
    compress: bool,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let sources = crate::query::FileSources {
        root: Path::new(root),
    };
    let request = crate::query::detail::BodyRequest {
        root,
        symbols: syms_spec,
        path_hint: None,
        start_line: None,
        end_line: None,
        outline: is_outline,
        compress,
    };
    crate::query::detail::execute_body_query(&request, ingest, interner, &sources, out)
}
