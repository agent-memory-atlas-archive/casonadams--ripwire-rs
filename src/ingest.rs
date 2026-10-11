use crate::crawler::CrawledFile;
use crate::domain::{
    FileInfo, IngestResult, Interner, Lang, NodeId, RefRole, Reference, SymKind, Symbol, NO_NODE,
};
use rayon::prelude::*;
use std::fs;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

/// Registry of supported Tree-Sitter languages and compiled queries.
pub struct LanguageRegistry {
    languages: Vec<(Lang, Language, Query)>,
}

impl LanguageRegistry {
    /// Initialize registry with Tier-1 languages: Rust, TypeScript, JavaScript, Python, Go.
    pub fn new_tier1() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let configs: [(Lang, Language, &str); 5] = [
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
        ];

        let mut languages = Vec::with_capacity(configs.len());
        for (lang, ts_lang, query_src) in configs {
            let query = Query::new(&ts_lang, query_src)?;
            languages.push((lang, ts_lang, query));
        }
        Ok(Self { languages })
    }

    /// Find compiled language and query for given Lang.
    pub fn get(&self, lang: Lang) -> Option<(&Language, &Query)> {
        self.languages
            .iter()
            .find(|(l, _, _)| *l == lang)
            .map(|(_, l, q)| (l, q))
    }
}

/// Raw definition extracted before deduplication and global ID assignment.
#[derive(Debug, Clone)]
struct RawDef {
    name: String,
    kind: SymKind,
    line: u32,
    start_byte: usize,
    end_byte: usize,
}

/// Raw reference extracted before enclosing symbol attribution.
#[derive(Debug, Clone)]
struct RawRef {
    callee_name: String,
    line: u32,
    start_byte: usize,
    role: RefRole,
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

/// Parse a single file content with a compiled grammar and query.
fn parse_file_content(source_bytes: &[u8], language: &Language, query: &Query) -> FileParseResult {
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
        let mut name: Option<String> = None;
        let mut def: Option<(SymKind, usize, usize, u32)> = None;
        let mut r#ref: Option<(RefRole, usize, u32)> = None;

        for capture in m.captures {
            let cap_name = query.capture_names()[capture.index as usize];
            let node = capture.node;
            match classify_capture(cap_name) {
                Some(Capture::Name) => {
                    if let Ok(text) = node.utf8_text(source_bytes) {
                        name = Some(text.to_string());
                    }
                }
                Some(Capture::Def(k)) => {
                    let line = node.start_position().row as u32 + 1;
                    def = Some((k, node.start_byte(), node.end_byte(), line));
                }
                Some(Capture::Ref(r)) => {
                    let line = node.start_position().row as u32 + 1;
                    r#ref = Some((r, node.start_byte(), line));
                }
                None => {}
            }
        }

        if let (Some(name), Some((kind, start, end, line))) = (name.as_ref(), def) {
            defs.push(RawDef {
                name: name.clone(),
                kind,
                line,
                start_byte: start,
                end_byte: end,
            });
        } else if let (Some(name), Some((role, start, line))) = (name, r#ref) {
            refs.push(RawRef {
                callee_name: name,
                line,
                start_byte: start,
                role,
            });
        }
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
            Some(parse_file_content(&source_bytes, language, query))
        })
        .collect();

    let mut ingest = IngestResult::new();
    let mut next_node_id: NodeId = 0;

    for (file_id, (file, parsed)) in files.iter().zip(parsed_files).enumerate() {
        let file_id = file_id as u32;
        let file_spur = interner.intern(&file.rel_path);

        let parse_res = match parsed {
            Some(res) => res,
            None => {
                ingest.files.push(FileInfo {
                    id: file_id,
                    path: file_spur,
                    lang: file.lang,
                    line_count: 0,
                    byte_size: file.byte_size,
                });
                continue;
            }
        };

        ingest.files.push(FileInfo {
            id: file_id,
            path: file_spur,
            lang: file.lang,
            line_count: parse_res.line_count,
            byte_size: parse_res.byte_size,
        });

        let mut file_symbols: Vec<Symbol> = Vec::with_capacity(parse_res.defs.len());

        for def in &parse_res.defs {
            let id = next_node_id;
            next_node_id += 1;

            let name_spur = interner.intern(&def.name);
            let canon_str = format!("{}::{}", file.rel_path, def.name);
            let canon_spur = interner.intern(&canon_str);

            let mut sym = Symbol::new(
                id, name_spur, canon_spur, def.kind, file.lang, file_id, def.line,
            );
            sym.sig_start_byte = def.start_byte as u32;
            sym.end_byte = def.end_byte as u32;
            file_symbols.push(sym);
        }

        for r in &parse_res.refs {
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
            reference.start_byte = r.start_byte as u32;
            ingest.references.push(reference);
        }

        ingest.symbols.extend(file_symbols);
    }

    ingest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier1_registry_and_parsing() {
        let reg = LanguageRegistry::new_tier1().unwrap();
        assert!(reg.get(Lang::Rust).is_some() && reg.get(Lang::Python).is_some());

        let (lang, query) = reg.get(Lang::Rust).unwrap();
        let res = parse_file_content(b"fn run() { log(); } fn log() {}", lang, query);
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
