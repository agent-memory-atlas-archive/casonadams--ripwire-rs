use crate::domain::{IngestResult, Interner, NodeId};
use crate::rank::PageRankResult;
use std::io::{self, Write};
use std::path::Path;

pub fn extract_symbol_body<'a>(
    sym: &'a crate::domain::Symbol,
    ingest: &'a IngestResult,
    interner: &'a Interner,
    root_path: &Path,
) -> Option<(&'a crate::domain::Symbol, &'a str, &'a str, String)> {
    let name = &interner[sym.name];
    let rel_path = ingest.sym_file_path(sym, interner);
    let bytes = std::fs::read(root_path.join(rel_path)).ok()?;
    let body = sym.source_text(&bytes)?.into_owned();
    Some((sym, name, rel_path, body))
}

pub fn skeletonize_body(body: &str) -> String {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut elided = false;
    for line in body.lines() {
        let opens = line.chars().filter(|&c| c == '{').count() as i32;
        let closes = line.chars().filter(|&c| c == '}').count() as i32;
        if depth <= 1 {
            out.push(line.to_string());
            elided = false;
        } else if !elided {
            out.push("    ...".to_string());
            elided = true;
        }
        depth += opens - closes;
        if depth <= 1 && closes > 0 && elided {
            out.push(line.to_string());
            elided = false;
        }
    }
    out.join("\n")
}

fn collect_expanded_symbol<'a>(
    id: NodeId,
    is_outline: bool,
    ingest: &'a IngestResult,
    interner: &'a Interner,
    root_path: &Path,
) -> Option<(&'a crate::domain::Symbol, &'a str, &'a str, String)> {
    let (sym, name, rel_path, body) =
        extract_symbol_body(&ingest.symbols[id as usize], ingest, interner, root_path)?;
    let final_body = if is_outline {
        skeletonize_body(&body)
    } else {
        body
    };
    Some((sym, name, rel_path, final_body))
}

fn collect_all_expanded_symbols<'a>(
    root: &str,
    syms_spec: &str,
    is_outline: bool,
    ingest: &'a IngestResult,
    interner: &'a Interner,
) -> io::Result<Vec<(&'a crate::domain::Symbol, &'a str, &'a str, String)>> {
    let mut expanded_items = Vec::new();
    let root_path = Path::new(root);
    for sym_name in syms_spec
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let ids = crate::navigate::find_symbols_by_name(sym_name, ingest, interner);
        if ids.is_empty() {
            let flag = if is_outline { "--outline" } else { "--expand" };
            return Err(io::Error::other(format!(
                "ripwire-rs: {flag} symbol not found: {sym_name}"
            )));
        }
        for &id in &ids {
            if let Some(item) = collect_expanded_symbol(id, is_outline, ingest, interner, root_path)
            {
                expanded_items.push(item);
            }
        }
    }
    Ok(expanded_items)
}

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
    let mut expanded_items =
        collect_all_expanded_symbols(root, syms_spec, is_outline, ingest, interner)?;
    if compress {
        for item in &mut expanded_items {
            item.3 = crate::clones::token_stream::compress_source_body(&item.3);
        }
    }
    let items_slice: Vec<(&crate::domain::Symbol, &str, &str, &str)> = expanded_items
        .iter()
        .map(|(s, n, p, b)| (*s, *n, *p, b.as_str()))
        .collect();
    crate::serialize::serialize_expand_xml(root, &items_slice, out)
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
