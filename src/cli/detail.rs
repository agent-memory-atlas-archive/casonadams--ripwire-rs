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

pub fn execute_tree_query<W: Write>(
    root: &str,
    ingest: &IngestResult,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (file_order, buckets) = crate::serialize::bucket_symbols_by_rank(
        &ingest.symbols,
        ingest.files.len(),
        &pr.ranks,
        None,
    );

    let mut file_symbols = Vec::new();
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    for f in file_order {
        let rel_path = view.file_path(f);
        let bucket = &buckets[f as usize];
        let mut top3 = Vec::new();
        for &sym_id in bucket.iter().take(3) {
            let sym = &ingest[sym_id];
            top3.push((sym.kind.tag(), view.sym_name(sym)));
        }
        file_symbols.push(crate::serialize::TreeFileSymbols {
            path: rel_path,
            total_symbols: bucket.len(),
            top_symbols: top3,
        });
    }

    use crate::serialize::XmlReport;
    let files_unlisted = ingest.files.len().saturating_sub(file_symbols.len());
    crate::serialize::query_xml::TreeReport {
        root,
        file_symbols: &file_symbols,
        files_unlisted,
        pr_iters: pr.iterations,
    }
    .render_envelope(out)
}
