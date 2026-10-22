use crate::domain::{IngestResult, Interner};
use crate::rank::PageRankResult;
use std::io::{self, Write};
use std::path::Path;

pub fn execute_expand_or_outline_query<W: Write>(
    root: &str,
    syms_spec: &str,
    is_outline: bool,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    execute_expand_or_outline_compressed(root, syms_spec, is_outline, false, ingest, interner, out)
}

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

pub fn execute_pack_top_n_query<W: Write>(
    root: &str,
    n: usize,
    ingest: &IngestResult,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (file_order, buckets) = crate::serialize::bucket_symbols_by_rank(
        &ingest.symbols,
        ingest.files.len(),
        &pr.ranks,
        Some(n),
    );
    let mut bodies = Vec::new();
    let mut count = 0;

    for f in file_order {
        if count >= n {
            break;
        }
        let rel_path = ingest.file_path(f, interner);
        if let Ok(bytes) = crate::domain::read_rel_bytes(root, rel_path) {
            let content = String::from_utf8_lossy(&bytes).to_string();
            bodies.push((rel_path, content));
            count += buckets[f as usize].len();
        }
    }

    let bodies_slice: Vec<(&str, &str)> = bodies.iter().map(|(p, c)| (*p, c.as_str())).collect();
    crate::serialize::serialize_pack_top_n_xml(root, &bodies_slice, out)
}

/// One `--tree` row per file with symbols, files ordered by their best-ranked symbol.
#[must_use]
pub fn tree_file_symbols<'a>(
    ingest: &'a IngestResult,
    pr: &PageRankResult,
    interner: &'a Interner,
) -> Vec<crate::serialize::TreeFileSymbols<'a>> {
    use crate::domain::CorpusView;
    let (file_order, buckets) = crate::serialize::bucket_symbols_by_rank(
        &ingest.symbols,
        ingest.files.len(),
        &pr.ranks,
        None,
    );
    let view = (ingest, interner);
    file_order
        .into_iter()
        .map(|f| {
            let bucket = &buckets[f as usize];
            crate::serialize::TreeFileSymbols {
                path: view.file_path(f),
                total_symbols: bucket.len(),
                top_symbols: bucket
                    .iter()
                    .take(3)
                    .map(|&id| (ingest[id].kind.tag(), view.sym_name(&ingest[id])))
                    .collect(),
            }
        })
        .collect()
}
