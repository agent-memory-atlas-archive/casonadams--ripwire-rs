use crate::crawler::CrawledFile;
use crate::domain::{
    FileInfo, IngestResult, Interner, Lang, NodeId, RefRole, Reference, SymKind, Symbol, NO_NODE,
};
use rayon::prelude::*;
use std::fs;
use std::sync::{Arc, LazyLock};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

static GLOBAL_REGISTRY: LazyLock<Vec<(Lang, Language, Arc<Query>)>> = LazyLock::new(|| {
    let configs: Vec<(Lang, Language, &str)> = vec![
        (
            Lang::Rust,
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("queries/rust/tags.scm"),
        ),
        (
            Lang::TypeScript,
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("queries/typescript/tags.scm"),
        ),
        (
            Lang::JavaScript,
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("queries/javascript/tags.scm"),
        ),
        (
            Lang::Python,
            tree_sitter_python::LANGUAGE.into(),
            include_str!("queries/python/tags.scm"),
        ),
        (
            Lang::Go,
            tree_sitter_go::LANGUAGE.into(),
            include_str!("queries/go/tags.scm"),
        ),
        (
            Lang::C,
            tree_sitter_c::LANGUAGE.into(),
            include_str!("queries/c/tags.scm"),
        ),
        (
            Lang::Cpp,
            tree_sitter_cpp::LANGUAGE.into(),
            include_str!("queries/cpp/tags.scm"),
        ),
        (
            Lang::Cuda,
            tree_sitter_cuda::LANGUAGE.into(),
            include_str!("queries/cuda/tags.scm"),
        ),
        (
            Lang::Metal,
            tree_sitter_cpp::LANGUAGE.into(),
            include_str!("queries/cpp/tags.scm"),
        ),
        (
            Lang::Java,
            tree_sitter_java::LANGUAGE.into(),
            include_str!("queries/java/tags.scm"),
        ),
        (
            Lang::CSharp,
            tree_sitter_c_sharp::LANGUAGE.into(),
            include_str!("queries/csharp/tags.scm"),
        ),
        (
            Lang::Swift,
            tree_sitter_swift::LANGUAGE.into(),
            include_str!("queries/swift/tags.scm"),
        ),
        (
            Lang::Bash,
            tree_sitter_bash::LANGUAGE.into(),
            include_str!("queries/bash/tags.scm"),
        ),
    ];

    let mut languages = Vec::with_capacity(configs.len());
    for (lang, ts_lang, query_src) in configs {
        let query = Query::new(&ts_lang, query_src).expect("valid grammar query");
        languages.push((lang, ts_lang, Arc::new(query)));
    }
    languages
});

/// Registry of supported Tree-Sitter languages and compiled queries.
pub struct LanguageRegistry {
    languages: Vec<(Lang, Language, Arc<Query>)>,
}

impl LanguageRegistry {
    /// Initialize registry with all supported languages across Tier-1 and Systems/Enterprise (Wave 1 & 2).
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            languages: GLOBAL_REGISTRY.clone(),
        })
    }

    /// Initialize registry with Tier-1 languages.
    pub fn new_tier1() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new()
    }

    /// Find compiled language and query for given Lang.
    pub fn get(&self, lang: Lang) -> Option<(&Language, &Query)> {
        self.languages
            .iter()
            .find(|(l, _, _)| *l == lang)
            .map(|(_, l, q)| (l, q.as_ref()))
    }
}

/// Raw definition extracted before deduplication and global ID assignment.
#[derive(Debug, Clone)]
struct RawDef {
    name: String,
    scope: Option<String>,
    kind: SymKind,
    line: u32,
    start_byte: usize,
    end_byte: usize,
    params: u16,
    metrics: crate::metrics::ComplexityMetrics,
}

/// Raw reference extracted before enclosing symbol attribution.
#[derive(Debug, Clone)]
struct RawRef {
    callee_name: String,
    qualifier: Option<String>,
    line: u32,
    start_byte: usize,
    role: RefRole,
    arg_count: u16,
    arg_count_known: bool,
}

/// Parsed facts from a single file.
#[derive(Debug, Default)]
struct FileParseResult {
    line_count: u32,
    byte_size: u64,
    defs: Vec<RawDef>,
    refs: Vec<RawRef>,
}

enum Capture {
    Name,
    Def(SymKind),
    Ref(RefRole),
}

fn classify_capture(name: &str) -> Option<Capture> {
    if name == "name" {
        return Some(Capture::Name);
    }
    if let Some(tail) = name.strip_prefix("definition.") {
        return Some(Capture::Def(match tail {
            "function" | "cjsexport" => SymKind::Function,
            "method" | "protomethod" => SymKind::Method,
            "class" => SymKind::Class,
            "struct" => SymKind::Struct,
            "interface" => SymKind::Interface,
            "var" | "constant" | "attribute" => SymKind::Var,
            "macro" => SymKind::Macro,
            "module" | "section" => SymKind::Section,
            "field" => SymKind::Field,
            _ => SymKind::Other,
        }));
    }
    name.strip_prefix("reference.").map(|tail| {
        Capture::Ref(match tail {
            "read" => RefRole::Read,
            "write" => RefRole::Write,
            "import" => RefRole::Import,
            "extends" | "implementation" => RefRole::Extends,
            "macro" => RefRole::Macro,
            "type" => RefRole::Type,
            "value" => RefRole::Value,
            "through" => RefRole::Through,
            _ => RefRole::Call,
        })
    })
}
fn extract_impl_scope(parent: tree_sitter::Node<'_>, source_bytes: &[u8]) -> Option<String> {
    let type_node = parent.child_by_field_name("type")?;
    let text = type_node.utf8_text(source_bytes).ok()?;
    let base = text.split('<').next().unwrap_or(text).trim();
    if base.is_empty() {
        None
    } else {
        Some(base.to_string())
    }
}

fn extract_named_container_scope(
    parent: tree_sitter::Node<'_>,
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> Option<String> {
    let name_child = parent.child_by_field_name("name")?;
    if name_child == name_node {
        return None;
    }
    let text = name_child.utf8_text(source_bytes).ok()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn extract_node_scope(
    parent: tree_sitter::Node<'_>,
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> Option<String> {
    match parent.kind() {
        "impl_item" => extract_impl_scope(parent, source_bytes),
        "mod_item" | "trait_item" | "class_declaration" | "class_definition"
        | "struct_specifier" => extract_named_container_scope(parent, name_node, source_bytes),
        _ => None,
    }
}

fn find_enclosing_scope(name_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> Option<String> {
    let mut curr = name_node.parent();
    while let Some(parent) = curr {
        if let Some(scope) = extract_node_scope(parent, name_node, source_bytes) {
            return Some(scope);
        }
        curr = parent.parent();
    }
    None
}

fn find_call_qualifier(name_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> Option<String> {
    let parent = name_node.parent()?;
    if parent.kind() == "scoped_identifier" {
        if let Some(path_node) = parent.child_by_field_name("path") {
            if let Ok(text) = path_node.utf8_text(source_bytes) {
                let last = text.rsplit("::").next().unwrap_or(text);
                let base = last.split('<').next().unwrap_or(last).trim();
                if !base.is_empty() {
                    return Some(base.to_string());
                }
            }
        }
    }
    None
}

fn count_arguments_in_list(child: tree_sitter::Node<'_>) -> u16 {
    let mut count = 0u16;
    for j in 0..child.named_child_count() {
        if let Some(arg) = child.named_child(j) {
            if !arg.kind().contains("comment") {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

fn find_arguments_child(parent: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    for i in 0..parent.child_count() {
        if let Some(child) = parent.child(i) {
            let ckind = child.kind();
            if ckind.contains("argument") || ckind == "args" {
                return Some(child);
            }
        }
    }
    None
}

fn count_call_arguments(name_node: tree_sitter::Node<'_>) -> (u16, bool) {
    let mut curr = name_node.parent();
    while let Some(parent) = curr {
        let kind = parent.kind();
        if kind.contains("call") || kind == "invocation" || kind == "method_invocation" {
            if let Some(args_node) = find_arguments_child(parent) {
                return (count_arguments_in_list(args_node), true);
            }
            break;
        }
        curr = parent.parent();
    }
    (0, false)
}

struct MatchExtract<'a> {
    name: Option<String>,
    name_node: Option<tree_sitter::Node<'a>>,
    def: Option<(SymKind, usize, usize, u32)>,
    def_node: Option<tree_sitter::Node<'a>>,
    r#ref: Option<(RefRole, usize, u32)>,
}

fn extract_match_captures<'a>(
    m: &tree_sitter::QueryMatch<'a, '_>,
    query: &Query,
    source_bytes: &[u8],
) -> MatchExtract<'a> {
    let mut ext = MatchExtract {
        name: None,
        name_node: None,
        def: None,
        def_node: None,
        r#ref: None,
    };
    for capture in m.captures {
        let cap_name = query.capture_names()[capture.index as usize];
        let node = capture.node;
        match classify_capture(cap_name) {
            Some(Capture::Name) => {
                if let Ok(text) = node.utf8_text(source_bytes) {
                    ext.name = Some(text.to_string());
                    ext.name_node = Some(node);
                }
            }
            Some(Capture::Def(k)) => {
                let line = node.start_position().row as u32 + 1;
                ext.def = Some((k, node.start_byte(), node.end_byte(), line));
                ext.def_node = Some(node);
            }
            Some(Capture::Ref(r)) => {
                let line = node.start_position().row as u32 + 1;
                ext.r#ref = Some((r, node.start_byte(), line));
            }
            None => {}
        }
    }
    ext
}

fn record_extracted_item(
    ext: MatchExtract<'_>,
    source_bytes: &[u8],
    lang: Lang,
    defs: &mut Vec<RawDef>,
    refs: &mut Vec<RawRef>,
) {
    if let (Some(name), Some((kind, start, end, line))) = (ext.name.as_ref(), ext.def) {
        let scope = ext
            .name_node
            .and_then(|n| find_enclosing_scope(n, source_bytes));
        let metrics = if kind == SymKind::Function || kind == SymKind::Method {
            ext.def_node
                .map(|n| crate::metrics::compute_complexity_from_node(n, source_bytes, lang))
                .unwrap_or_default()
        } else {
            crate::metrics::ComplexityMetrics::default()
        };
        let params = ext
            .def_node
            .map(crate::gate::contract::extract_param_count)
            .unwrap_or(0);
        defs.push(RawDef {
            name: name.clone(),
            scope,
            kind,
            line,
            start_byte: start,
            end_byte: end,
            params,
            metrics,
        });
    } else if let (Some(name), Some((role, start, line))) = (ext.name, ext.r#ref) {
        let qualifier = ext
            .name_node
            .and_then(|n| find_call_qualifier(n, source_bytes));
        let (arg_count, arg_count_known) = ext
            .name_node
            .map(count_call_arguments)
            .unwrap_or((0, false));
        refs.push(RawRef {
            callee_name: name,
            qualifier,
            line,
            start_byte: start,
            role,
            arg_count,
            arg_count_known,
        });
    }
}

/// Parse a single file content with a compiled grammar and query.
fn parse_file_content(
    source_bytes: &[u8],
    language: &Language,
    query: &Query,
    lang: Lang,
) -> FileParseResult {
    let mut parser = Parser::new();
    if parser.set_language(language).is_err() {
        return FileParseResult::default();
    }

    let tree = match parser.parse(source_bytes, None) {
        Some(t) => t,
        None => return FileParseResult::default(),
    };

    let line_count = source_bytes.iter().filter(|&&b| b == b'\n').count() as u32 + 1;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

    let mut defs = Vec::new();
    let mut refs = Vec::new();

    while let Some(m) = matches.next() {
        let ext = extract_match_captures(m, query, source_bytes);
        record_extracted_item(ext, source_bytes, lang, &mut defs, &mut refs);
    }

    defs.sort_by(|a, b| {
        a.start_byte
            .cmp(&b.start_byte)
            .then_with(|| a.name.cmp(&b.name))
    });
    defs.dedup_by(|a, b| a.start_byte == b.start_byte && a.name == b.name);

    FileParseResult {
        line_count,
        byte_size: source_bytes.len() as u64,
        defs,
        refs,
    }
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
            Some(parse_file_content(
                &source_bytes,
                language,
                query,
                file.lang,
            ))
        })
        .collect();

    let mut ingest = IngestResult::new();
    let mut next_node_id: NodeId = 0;

    for (file_id, (file, parsed)) in files.iter().zip(parsed_files).enumerate() {
        let file_id = file_id as u32;
        let file_spur = interner.intern(&file.rel_path);
        let (line_count, byte_size, defs, refs) = match parsed {
            Some(p) => (p.line_count, p.byte_size, p.defs, p.refs),
            None => (0, file.byte_size, Vec::new(), Vec::new()),
        };

        ingest.files.push(FileInfo {
            id: file_id,
            path: file_spur,
            lang: file.lang,
            line_count,
            byte_size,
        });

        let file_symbols = build_file_symbols(defs, file, file_id, interner, &mut next_node_id);
        let file_refs = build_file_references(refs, file, file_id, &file_symbols, interner);

        ingest.references.extend(file_refs);
        ingest.symbols.extend(file_symbols);
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

        let parsed = registry
            .get(lang)
            .map(|(language, query)| parse_file_content(source_bytes, language, query, lang));

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

fn build_file_symbols(
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

fn build_file_references(
    refs: Vec<RawRef>,
    file: &CrawledFile,
    file_id: u32,
    file_symbols: &[Symbol],
    interner: &Interner,
) -> Vec<Reference> {
    refs.into_iter()
        .map(|r| {
            let from_symbol = file_symbols
                .iter()
                .filter(|s| {
                    (s.sig_start_byte as usize) <= r.start_byte
                        && r.start_byte <= (s.end_byte as usize)
                })
                .min_by_key(|s| s.end_byte - s.sig_start_byte)
                .map(|s| s.id)
                .unwrap_or(NO_NODE);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier1_registry_and_parsing() {
        let reg = LanguageRegistry::new_tier1().unwrap();
        assert!(reg.get(Lang::Rust).is_some() && reg.get(Lang::Python).is_some());

        let (lang, query) = reg.get(Lang::Rust).unwrap();
        let res = parse_file_content(b"fn run() { log(); } fn log() {}", lang, query, Lang::Rust);
        assert_eq!(res.defs.len(), 2);
        assert_eq!(res.refs.len(), 1);
        assert_eq!(res.refs[0].callee_name, "log");
    }

    #[test]
    fn test_classify_captures() {
        assert!(matches!(classify_capture("name"), Some(Capture::Name)));
        assert!(classify_capture("other").is_none());
        for tail in [
            "function",
            "cjsexport",
            "method",
            "protomethod",
            "class",
            "struct",
            "interface",
            "var",
            "macro",
            "module",
            "field",
            "x",
        ] {
            assert!(classify_capture(&format!("definition.{tail}")).is_some());
        }
        for tail in [
            "read", "write", "import", "extends", "macro", "type", "value", "through", "call", "y",
        ] {
            assert!(classify_capture(&format!("reference.{tail}")).is_some());
        }
    }
}
