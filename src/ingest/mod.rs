use crate::crawler::CrawledFile;
use crate::domain::{FileInfo, IngestResult, Lang, RefRole, Reference, SymKind, Symbol, NO_NODE};
use crate::interner::Interner;
use rayon::prelude::*;
use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

/// Definition of an extractable language using Tree-Sitter.
pub struct LanguageDefinition {
    pub lang: Lang,
    pub language_fn: Language,
    pub query_source: &'static str,
}

impl LanguageDefinition {
    pub fn new(lang: Lang, language_fn: Language, query_source: &'static str) -> Self {
        Self {
            lang,
            language_fn,
            query_source,
        }
    }
}

/// Registry of supported Tree-Sitter languages and compiled queries.
pub struct LanguageRegistry {
    languages: Vec<(Lang, Language, Query)>,
}

impl LanguageRegistry {
    /// Initialize registry with Tier-1 languages: Rust, TypeScript, JavaScript, Python, Go.
    pub fn new_tier1() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut reg = Self {
            languages: Vec::new(),
        };

        // 1. Rust
        let rust_lang: Language = tree_sitter_rust::LANGUAGE.into();
        let rust_query = Query::new(&rust_lang, include_str!("queries/rust/tags.scm"))?;
        reg.languages.push((Lang::Rust, rust_lang, rust_query));

        // 2. TypeScript
        let ts_lang: Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let ts_query = Query::new(&ts_lang, include_str!("queries/typescript/tags.scm"))?;
        reg.languages.push((Lang::TypeScript, ts_lang, ts_query));

        // 3. JavaScript
        let js_lang: Language = tree_sitter_javascript::LANGUAGE.into();
        let js_query = Query::new(&js_lang, include_str!("queries/javascript/tags.scm"))?;
        reg.languages.push((Lang::JavaScript, js_lang, js_query));

        // 4. Python
        let py_lang: Language = tree_sitter_python::LANGUAGE.into();
        let py_query = Query::new(&py_lang, include_str!("queries/python/tags.scm"))?;
        reg.languages.push((Lang::Python, py_lang, py_query));

        // 5. Go
        let go_lang: Language = tree_sitter_go::LANGUAGE.into();
        let go_query = Query::new(&go_lang, include_str!("queries/go/tags.scm"))?;
        reg.languages.push((Lang::Go, go_lang, go_query));

        Ok(reg)
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

/// Map capture name (`definition.function`, `reference.call`, etc.) to (Role, SymKind, RefRole).
enum CaptureClassification {
    Name,
    Definition(SymKind),
    Reference(RefRole),
    Ignore,
}

fn classify_capture(name: &str) -> CaptureClassification {
    if name == "name" {
        return CaptureClassification::Name;
    }

    if let Some(tail) = name.strip_prefix("definition.") {
        let kind = match tail {
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
        };
        return CaptureClassification::Definition(kind);
    }

    if let Some(tail) = name.strip_prefix("reference.") {
        let role = match tail {
            "call" => RefRole::Call,
            "read" => RefRole::Read,
            "write" => RefRole::Write,
            "import" => RefRole::Import,
            "extends" | "implementation" => RefRole::Extends,
            "macro" => RefRole::Macro,
            "type" => RefRole::Type,
            "value" => RefRole::Value,
            "through" => RefRole::Through,
            _ => RefRole::Call,
        };
        return CaptureClassification::Reference(role);
    }

    CaptureClassification::Ignore
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
        let mut name_text: Option<String> = None;
        let mut def_kind: Option<SymKind> = None;
        let mut def_span: Option<(usize, usize, u32)> = None;
        let mut ref_role: Option<RefRole> = None;
        let mut ref_span: Option<(usize, u32)> = None;

        for capture in m.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            let node = capture.node;

            match classify_capture(capture_name) {
                CaptureClassification::Name => {
                    if let Ok(text) = node.utf8_text(source_bytes) {
                        name_text = Some(text.to_string());
                    }
                }
                CaptureClassification::Definition(kind) => {
                    def_kind = Some(kind);
                    let line = node.start_position().row as u32 + 1;
                    def_span = Some((node.start_byte(), node.end_byte(), line));
                }
                CaptureClassification::Reference(role) => {
                    ref_role = Some(role);
                    let line = node.start_position().row as u32 + 1;
                    ref_span = Some((node.start_byte(), line));
                }
                CaptureClassification::Ignore => {}
            }
        }

        if let (Some(name), Some(kind), Some((start, end, line))) =
            (name_text.as_ref(), def_kind, def_span)
        {
            defs.push(RawDef {
                name: name.clone(),
                kind,
                line,
                start_byte: start,
                end_byte: end,
            });
        } else if let (Some(name), Some(role), Some((start, line))) =
            (name_text, ref_role, ref_span)
        {
            refs.push(RawRef {
                callee_name: name,
                line,
                start_byte: start,
                role,
            });
        }
    }

    // Deduplicate definitions sharing the same (start_byte, name)
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
    // 1. Parallel parse phase: parse each file into raw definitions and references
    let parsed_files: Vec<Option<FileParseResult>> = files
        .par_iter()
        .enumerate()
        .map(|(_id, file)| {
            let (language, query) = registry.get(file.lang)?;
            let source_bytes = fs::read(&file.path).ok()?;
            Some(parse_file_content(&source_bytes, language, query))
        })
        .collect();

    // 2. Sequential model assembly phase: assign stable global NodeIds in (file_id, line, name) order
    let mut ingest = IngestResult::new();
    let next_node_id = AtomicU32::new(0);

    for (file_id, file) in files.iter().enumerate() {
        let file_spur = interner.intern(&file.rel_path);
        let parse_res = match parsed_files.get(file_id).and_then(|p| p.as_ref()) {
            Some(res) => res,
            None => {
                ingest.files.push(FileInfo {
                    id: file_id as u32,
                    path: file_spur,
                    lang: file.lang,
                    line_count: 0,
                    byte_size: file.byte_size,
                });
                continue;
            }
        };

        ingest.files.push(FileInfo {
            id: file_id as u32,
            path: file_spur,
            lang: file.lang,
            line_count: parse_res.line_count,
            byte_size: parse_res.byte_size,
        });

        // Track file's definitions and their byte spans for reference enclosing-symbol attribution
        let mut file_symbols: Vec<Symbol> = Vec::with_capacity(parse_res.defs.len());

        for def in &parse_res.defs {
            let id = next_node_id.fetch_add(1, Ordering::SeqCst);
            let name_spur = interner.intern(&def.name);
            let canon_str = format!("{}::{}", file.rel_path, def.name);
            let canon_spur = interner.intern(&canon_str);

            let mut sym = Symbol::new(
                id,
                name_spur,
                canon_spur,
                def.kind,
                file.lang,
                file_id as u32,
                def.line,
            );
            sym.sig_start_byte = def.start_byte as u32;
            sym.end_byte = def.end_byte as u32;
            file_symbols.push(sym);
        }

        // Attribute references to enclosing symbols by byte containment
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
            let mut reference = Reference::new(
                from_symbol,
                callee_spur,
                file_id as u32,
                r.line,
                file.lang,
                r.role,
            );
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
    fn test_tier1_registry_initialization() {
        let registry = LanguageRegistry::new_tier1().unwrap();
        assert!(registry.get(Lang::Rust).is_some());
        assert!(registry.get(Lang::TypeScript).is_some());
        assert!(registry.get(Lang::JavaScript).is_some());
        assert!(registry.get(Lang::Python).is_some());
        assert!(registry.get(Lang::Go).is_some());
        assert!(registry.get(Lang::Ruby).is_none());
    }

    #[test]
    fn test_parse_rust_file() {
        let registry = LanguageRegistry::new_tier1().unwrap();
        let (language, query) = registry.get(Lang::Rust).unwrap();
        let code = r#"
            pub struct Engine {
                name: String,
            }

            impl Engine {
                pub fn run(&self) {
                    helper();
                }
            }

            fn helper() {}
        "#;

        let res = parse_file_content(code.as_bytes(), language, query);
        assert_eq!(res.defs.len(), 3); // Engine, run, helper
        let names: Vec<&str> = res.defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"Engine"));
        assert!(names.contains(&"run"));
        assert!(names.contains(&"helper"));

        assert_eq!(res.refs.len(), 1); // helper() call
        assert_eq!(res.refs[0].callee_name, "helper");
        assert_eq!(res.refs[0].role, RefRole::Call);
    }

    #[test]
    fn test_parse_python_file() {
        let registry = LanguageRegistry::new_tier1().unwrap();
        let (language, query) = registry.get(Lang::Python).unwrap();
        let code = r#"
class Agent:
    def execute(self):
        log_event()

def log_event():
    pass
"#;

        let res = parse_file_content(code.as_bytes(), language, query);
        let names: Vec<&str> = res.defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"Agent"));
        assert!(names.contains(&"execute"));
        assert!(names.contains(&"log_event"));
    }
}
