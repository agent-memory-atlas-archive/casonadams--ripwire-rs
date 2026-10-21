use super::extract::{FileParseResult, RawDef, RawRef, parse_file_tree};
use super::ownership::{assign_module_scope_owner, build_file_references};
use super::registry::LanguageRegistry;
use crate::crawler::CrawledFile;
use crate::domain::{FileInfo, IngestResult, Interner, Lang, NodeId, Reference, Symbol};
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
    if let Some(overridden) =
        lang.semantics()
            .parse_file_override(source_bytes, rel_path, language, query)
    {
        return overridden;
    }
    parse_file_tree(source_bytes, language, query, lang)
}

/// Lines where `name` is defined in `source_bytes`, by the tags query of `rel_path`'s language.
#[must_use]
pub fn definition_lines(rel_path: &str, source_bytes: &[u8], name: &str) -> Vec<u32> {
    let lang = Lang::from_path(std::path::Path::new(rel_path));
    let lang = lang
        .semantics()
        .refine_dialect(lang, rel_path, source_bytes);
    let Some((language, query)) = LanguageRegistry.get(lang) else {
        return Vec::new();
    };
    parse_file_content(source_bytes, language, query, lang, rel_path)
        .defs
        .into_iter()
        .filter(|d| d.name == name)
        .map(|d| d.line)
        .collect()
}

/// Ingest crawled files in parallel and assemble IngestResult.
pub fn ingest_crawled_files(
    files: &[CrawledFile],
    registry: &LanguageRegistry,
    interner: &Interner,
) -> IngestResult {
    let parsed_files: Vec<Option<(Lang, FileParseResult)>> = files
        .par_iter()
        .map(|file| {
            let source_bytes = fs::read(&file.path).ok()?;
            if crate::crawler::is_binary_buffer(&source_bytes) {
                return None;
            }
            let eff_lang =
                file.lang
                    .semantics()
                    .refine_dialect(file.lang, &file.rel_path, &source_bytes);
            let (language, query) = registry.get(eff_lang)?;
            Some((
                eff_lang,
                parse_file_content(&source_bytes, language, query, eff_lang, &file.rel_path),
            ))
        })
        .collect();

    let processed: Vec<(FileInfo, FileIndex)> = (0..files.len())
        .into_par_iter()
        .zip(parsed_files)
        .map(|(file_id, parsed)| {
            let file = &files[file_id];
            let file_id = file_id as u32;
            let (eff_lang, line_count, byte_size, defs, refs) = match parsed {
                Some((l, p)) => (l, p.line_count, p.byte_size, p.defs, p.refs),
                None => (file.lang, 0, file.byte_size, Vec::new(), Vec::new()),
            };
            let file_info = FileInfo {
                id: file_id,
                path: interner.intern(&file.rel_path),
                lang: eff_lang,
                line_count,
                byte_size,
            };
            let mut eff_file = file.clone();
            eff_file.lang = eff_lang;
            (
                file_info,
                index_file(defs, refs, &eff_file, file_id, interner),
            )
        })
        .collect();

    let mut ingest = IngestResult::new();
    ingest.files.reserve(files.len());
    for (file_info, index) in processed {
        append_file(&mut ingest, file_info, index);
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

    for (file_id, &(rel_path, lang, source_bytes)) in files.iter().enumerate() {
        let file_id = file_id as u32;
        let eff_lang = lang
            .semantics()
            .refine_dialect(lang, rel_path, source_bytes);
        let parsed = registry.get(eff_lang).map(|(language, query)| {
            parse_file_content(source_bytes, language, query, eff_lang, rel_path)
        });

        let (line_count, byte_size, defs, refs) = match parsed {
            Some(p) => (p.line_count, p.byte_size, p.defs, p.refs),
            None => (0, source_bytes.len() as u64, Vec::new(), Vec::new()),
        };
        let file_info = FileInfo {
            id: file_id,
            path: interner.intern(rel_path),
            lang: eff_lang,
            line_count,
            byte_size,
        };
        let dummy_crawled = CrawledFile {
            path: std::path::PathBuf::from(rel_path),
            rel_path: rel_path.to_string(),
            lang: eff_lang,
            byte_size: source_bytes.len() as u64,
        };
        let index = index_file(defs, refs, &dummy_crawled, file_id, interner);
        append_file(&mut ingest, file_info, index);
    }

    ingest
}

/// One file's symbols and references, with node ids local to the file (starting at 0).
pub struct FileIndex {
    pub symbols: Vec<Symbol>,
    pub references: Vec<Reference>,
}

pub fn index_file(
    defs: Vec<RawDef>,
    refs: Vec<RawRef>,
    file: &CrawledFile,
    file_id: u32,
    interner: &Interner,
) -> FileIndex {
    let mut next_id: NodeId = 0;
    let mut symbols = build_file_symbols(defs, file, file_id, interner, &mut next_id);
    let mut references = build_file_references(refs, file, file_id, &symbols, interner);
    assign_module_scope_owner(
        &mut symbols,
        &mut references,
        file,
        file_id,
        interner,
        &mut next_id,
    );
    FileIndex {
        symbols,
        references,
    }
}

/// Append a file, rebasing its file-local node ids onto the global symbol table.
fn append_file(ingest: &mut IngestResult, file_info: FileInfo, mut index: FileIndex) {
    let offset = ingest.symbols.len() as NodeId;
    for s in &mut index.symbols {
        s.id += offset;
    }
    for r in &mut index.references {
        r.from_symbol += offset;
    }
    ingest.files.push(file_info);
    ingest.symbols.append(&mut index.symbols);
    ingest.references.append(&mut index.references);
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
        sym.sig_end_byte = if def.body_byte > def.start_byte {
            def.body_byte as u32
        } else {
            def.end_byte as u32
        };
        sym.end_byte = def.end_byte as u32;
        sym.loc = def.loc.max(1);
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
