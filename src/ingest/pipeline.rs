use super::extract::{FileParseResult, RawDef, RawRef, parse_file_tree};
use super::registry::LanguageRegistry;
use super::{astro, markdown, yaml_guard};
use crate::crawler::CrawledFile;
use crate::domain::{FileInfo, IngestResult, Interner, Lang, NO_NODE, NodeId, Reference, Symbol};
use rayon::prelude::*;
use std::fs;
use tree_sitter::{Language, Query};

/// Parse a single file content with a compiled grammar and query.
pub fn parse_file_content(
    source_bytes: &[u8],
    language: &Language,
    query: &Query,
    lang: Lang,
    rel_path: &str,
) -> FileParseResult {
    if lang == Lang::Markdown {
        return markdown::extract_markdown(source_bytes, rel_path, language);
    }

    if lang == Lang::Yaml && yaml_guard::yaml_nests_too_deep(source_bytes) {
        return FileParseResult::default();
    }

    if rel_path.ends_with(".astro") {
        return match astro::extract_astro_frontmatter(source_bytes) {
            astro::AstroFrontmatter::Ok {
                start_byte,
                end_byte,
                start_row,
            } => {
                let sub_bytes = &source_bytes[start_byte..end_byte];
                let mut res = parse_file_tree(sub_bytes, language, query, Lang::TypeScript);
                for d in &mut res.defs {
                    d.line += start_row;
                    d.start_byte += start_byte;
                    d.end_byte += start_byte;
                }
                for r in &mut res.refs {
                    r.line += start_row;
                    r.start_byte += start_byte;
                }
                res.line_count = source_bytes.iter().filter(|&&b| b == b'\n').count() as u32 + 1;
                res.byte_size = source_bytes.len() as u64;
                res
            }
            _ => FileParseResult::default(),
        };
    }

    parse_file_tree(source_bytes, language, query, lang)
}

/// Ingest crawled files in parallel and assemble IngestResult.
pub fn ingest_crawled_files(
    files: &[CrawledFile],
    registry: &LanguageRegistry,
    interner: &Interner,
) -> IngestResult {
    let parsed_files: Vec<Option<FileParseResult>> = files
        .par_iter()
        .map(|file| {
            let (language, query) = registry.get(file.lang)?;
            let source_bytes = fs::read(&file.path).ok()?;
            if crate::crawler::is_binary_buffer(&source_bytes) {
                return None;
            }
            Some(parse_file_content(
                &source_bytes,
                language,
                query,
                file.lang,
                &file.rel_path,
            ))
        })
        .collect();

    let mut node_offsets = Vec::with_capacity(files.len() + 1);
    let mut current_offset: NodeId = 0;
    for parsed in &parsed_files {
        node_offsets.push(current_offset);
        if let Some(p) = parsed {
            current_offset += p.defs.len() as NodeId;
        }
    }
    node_offsets.push(current_offset);

    let processed: Vec<(FileInfo, Vec<Symbol>, Vec<Reference>)> = (0..files.len())
        .into_par_iter()
        .zip(parsed_files)
        .map(|(file_id, parsed)| {
            let file = &files[file_id];
            let file_id = file_id as u32;
            let file_spur = interner.intern(&file.rel_path);
            let mut start_id = node_offsets[file_id as usize];

            let (line_count, byte_size, defs, refs) = match parsed {
                Some(p) => (p.line_count, p.byte_size, p.defs, p.refs),
                None => (0, file.byte_size, Vec::new(), Vec::new()),
            };

            let file_info = FileInfo {
                id: file_id,
                path: file_spur,
                lang: file.lang,
                line_count,
                byte_size,
            };

            let file_symbols = build_file_symbols(defs, file, file_id, interner, &mut start_id);
            let file_refs = build_file_references(refs, file, file_id, &file_symbols, interner);

            (file_info, file_symbols, file_refs)
        })
        .collect();

    let mut ingest = IngestResult::new();
    ingest.files.reserve(files.len());
    ingest.symbols.reserve(current_offset as usize);

    for (file_info, file_symbols, file_refs) in processed {
        ingest.files.push(file_info);
        ingest.symbols.extend(file_symbols);
        ingest.references.extend(file_refs);
    }
    ingest
}

/// Ingest in-memory source files directly without filesystem I/O.
pub fn ingest_in_memory_files(
    files: &[(&str, Lang, &[u8])],
    registry: &LanguageRegistry,
    interner: &Interner,
) -> IngestResult {
    let mut ingest = IngestResult::new();
    let mut next_node_id: NodeId = 0;

    for (file_id, &(rel_path, lang, source_bytes)) in files.iter().enumerate() {
        let file_id = file_id as u32;
        let file_spur = interner.intern(rel_path);

        let parsed = registry.get(lang).map(|(language, query)| {
            parse_file_content(source_bytes, language, query, lang, rel_path)
        });

        let (line_count, byte_size, defs, refs) = match parsed {
            Some(p) => (p.line_count, p.byte_size, p.defs, p.refs),
            None => (0, source_bytes.len() as u64, Vec::new(), Vec::new()),
        };

        ingest.files.push(FileInfo {
            id: file_id,
            path: file_spur,
            lang,
            line_count,
            byte_size,
        });

        let dummy_crawled = CrawledFile {
            path: std::path::PathBuf::from(rel_path),
            rel_path: rel_path.to_string(),
            lang,
            byte_size: source_bytes.len() as u64,
        };

        let file_symbols =
            build_file_symbols(defs, &dummy_crawled, file_id, interner, &mut next_node_id);
        let file_refs =
            build_file_references(refs, &dummy_crawled, file_id, &file_symbols, interner);

        ingest.references.extend(file_refs);
        ingest.symbols.extend(file_symbols);
    }

    ingest
}

pub fn build_file_symbols(
    defs: Vec<RawDef>,
    file: &CrawledFile,
    file_id: u32,
    interner: &Interner,
    next_node_id: &mut NodeId,
) -> Vec<Symbol> {
    let mut file_symbols = Vec::with_capacity(defs.len());
    for def in defs {
        let id = *next_node_id;
        *next_node_id += 1;
        let name = interner.intern(&def.name);
        let canon = match &def.scope {
            Some(sc) => interner.intern(&format!("{}::{}::{}", file.rel_path, sc, def.name)),
            None => interner.intern(&format!("{}::{}", file.rel_path, def.name)),
        };
        let mut sym = Symbol::new(id, name, canon, def.kind, file.lang, file_id, def.line);
        if let Some(sc) = &def.scope {
            sym.scope = Some(interner.intern(sc));
        }
        sym.sig_start_byte = def.start_byte as u32;
        sym.end_byte = def.end_byte as u32;
        sym.cx = def.metrics.cx;
        sym.ccx = def.metrics.ccx;
        sym.max_nest = def.metrics.max_nest;
        sym.params = def.params;
        sym.humps = def.metrics.humps;
        sym.deep = def.metrics.deep;
        sym.ev = def.metrics.ev;
        file_symbols.push(sym);
    }
    file_symbols
}

pub fn find_enclosing_symbol(file_symbols: &[Symbol], start_byte: usize) -> NodeId {
    let upper = file_symbols.partition_point(|s| (s.sig_start_byte as usize) <= start_byte);
    let mut best_id = NO_NODE;
    let mut best_span = u32::MAX;

    for s in file_symbols[..upper].iter().rev() {
        let sig_start = s.sig_start_byte as usize;
        let dist = start_byte - sig_start;
        if dist as u32 >= best_span {
            break;
        }
        if start_byte <= (s.end_byte as usize) {
            let span = s.end_byte - s.sig_start_byte;
            if span < best_span {
                best_span = span;
                best_id = s.id;
            }
        }
    }
    best_id
}

pub fn build_file_references(
    refs: Vec<RawRef>,
    file: &CrawledFile,
    file_id: u32,
    file_symbols: &[Symbol],
    interner: &Interner,
) -> Vec<Reference> {
    refs.into_iter()
        .map(|r| {
            let from_symbol = find_enclosing_symbol(file_symbols, r.start_byte);

            let callee_spur = interner.intern(&r.callee_name);
            let mut reference =
                Reference::new(from_symbol, callee_spur, file_id, r.line, file.lang, r.role);
            if let Some(q) = &r.qualifier {
                reference.qualifier = Some(interner.intern(q));
            }
            reference.start_byte = r.start_byte as u32;
            reference.arg_count = r.arg_count;
            reference.arg_count_known = r.arg_count_known;
            reference
        })
        .collect()
}
