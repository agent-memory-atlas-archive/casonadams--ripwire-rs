use ignore::WalkBuilder;
use lasso::{Spur, ThreadedRodeo};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

// --- DOMAIN TYPES ---

pub type NodeId = u32;
pub const NO_NODE: NodeId = u32::MAX;
pub const DEFAULT_MAX_FILE_SIZE: u64 = 4 * 1024 * 1024;

pub const DENYLIST_DIRS: &[&str] = &[
    ".git",
    ".claude",
    ".hg",
    ".svn",
    "node_modules",
    "vendor",
    "third_party",
    ".cache",
    "build",
    "dist",
    "out",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    ".idea",
    ".vscode",
    "asan",
    "build_prof",
    "CMakeFiles",
    "captures",
];

/// Thread-safe string interner backed by `lasso::ThreadedRodeo`.
#[derive(Debug, Clone, Default)]
pub struct Interner(Arc<ThreadedRodeo>);

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }
    #[inline]
    pub fn intern(&self, s: &str) -> Spur {
        self.0.get_or_intern(s)
    }
    #[inline]
    pub fn resolve<'a>(&'a self, spur: &Spur) -> &'a str {
        self.0.resolve(spur)
    }
    #[inline]
    pub fn try_resolve<'a>(&'a self, spur: &Spur) -> Option<&'a str> {
        self.0.try_resolve(spur)
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymKind {
    Function = 0,
    Method,
    Class,
    Struct,
    Interface,
    Var,
    Section,
    Macro,
    Field,
    Other,
    ModuleScope,
}

impl SymKind {
    #[inline]
    pub const fn tag(self) -> &'static str {
        const TAGS: [&str; 11] = [
            "fn", "method", "cls", "struct", "iface", "var", "sec", "macro", "field", "other",
            "modscope",
        ];
        TAGS[self as usize]
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RefRole {
    Call = 0,
    Read,
    Write,
    Import,
    Extends,
    Macro,
    Type,
    Value,
    Through,
}

impl RefRole {
    #[inline]
    pub const fn tag(self) -> &'static str {
        const TAGS: [&str; 9] = [
            "call", "read", "write", "import", "extends", "macro", "type", "value", "through",
        ];
        TAGS[self as usize]
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lang {
    Rust = 0,
    TypeScript,
    JavaScript,
    Python,
    Go,
    C,
    Cpp,
    Cuda,
    Metal,
    Java,
    CSharp,
    Swift,
    Bash,
    Ruby,
    Php,
    Lua,
    Elixir,
    Dart,
    Kotlin,
    GDScript,
    Json,
    Toml,
    Yaml,
    Markdown,
    Unknown,
}

impl Lang {
    #[inline]
    pub const fn tag(self) -> &'static str {
        const TAGS: [&str; 25] = [
            "rs", "ts", "js", "py", "go", "c", "cpp", "cuda", "metal", "java", "cs", "swift", "sh",
            "rb", "php", "lua", "ex", "dart", "kt", "gd", "json", "toml", "yaml", "md", "?",
        ];
        TAGS[self as usize]
    }

    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "rs" => Self::Rust,
            "ts" | "tsx" | "mts" | "cts" => Self::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "py" | "pyi" => Self::Python,
            "go" => Self::Go,
            "c" | "h" => Self::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Self::Cpp,
            "cu" | "cuh" => Self::Cuda,
            "metal" => Self::Metal,
            "java" => Self::Java,
            "cs" => Self::CSharp,
            "swift" => Self::Swift,
            "sh" | "bash" => Self::Bash,
            "rb" => Self::Ruby,
            "php" | "phtml" => Self::Php,
            "lua" => Self::Lua,
            "ex" | "exs" => Self::Elixir,
            "dart" => Self::Dart,
            "kt" | "kts" => Self::Kotlin,
            "gd" => Self::GDScript,
            "json" => Self::Json,
            "toml" => Self::Toml,
            "yml" | "yaml" => Self::Yaml,
            "md" | "markdown" => Self::Markdown,
            _ => Self::Unknown,
        }
    }

    pub fn is_compatible(self, other: Self) -> bool {
        self == other
            || matches!(
                (self, other),
                (Self::C, Self::Cpp)
                    | (Self::Cpp, Self::C)
                    | (Self::TypeScript, Self::JavaScript)
                    | (Self::JavaScript, Self::TypeScript)
                    | (Self::Java, Self::Kotlin)
                    | (Self::Kotlin, Self::Java)
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    pub id: NodeId,
    pub name: Spur,
    pub canonical_id: Spur,
    pub kind: SymKind,
    pub lang: Lang,
    pub file_id: u32,
    pub line: u32,
    pub sig_start_byte: u32,
    pub sig_end_byte: u32,
    pub end_byte: u32,
    pub cx: u32,
    pub ccx: u32,
    pub loc: u32,
    pub max_nest: u16,
    pub params: u16,
}

impl Symbol {
    pub fn new(
        id: NodeId,
        name: Spur,
        canonical_id: Spur,
        kind: SymKind,
        lang: Lang,
        file_id: u32,
        line: u32,
    ) -> Self {
        Self {
            id,
            name,
            canonical_id,
            kind,
            lang,
            file_id,
            line,
            sig_start_byte: 0,
            sig_end_byte: 0,
            end_byte: 0,
            cx: 1,
            ccx: 0,
            loc: 1,
            max_nest: 0,
            params: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub from_symbol: NodeId,
    pub callee_name: Spur,
    pub file_id: u32,
    pub line: u32,
    pub start_byte: u32,
    pub lang: Lang,
    pub role: RefRole,
    pub arg_count: u16,
    pub arg_count_known: bool,
}

impl Reference {
    pub fn new(
        from_symbol: NodeId,
        callee_name: Spur,
        file_id: u32,
        line: u32,
        lang: Lang,
        role: RefRole,
    ) -> Self {
        Self {
            from_symbol,
            callee_name,
            file_id,
            line,
            start_byte: 0,
            lang,
            role,
            arg_count: 0,
            arg_count_known: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: u32,
    pub path: Spur,
    pub lang: Lang,
    pub line_count: u32,
    pub byte_size: u64,
}

#[derive(Debug, Default, Clone)]
pub struct IngestResult {
    pub files: Vec<FileInfo>,
    pub symbols: Vec<Symbol>,
    pub references: Vec<Reference>,
}

impl IngestResult {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }
    pub fn reference_count(&self) -> usize {
        self.references.len()
    }
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

// --- CRAWLER ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawledFile {
    pub path: PathBuf,
    pub rel_path: String,
    pub lang: Lang,
    pub byte_size: u64,
}

#[derive(Debug, Clone)]
pub struct CrawlerOptions {
    pub max_file_size: u64,
    pub honor_gitignore: bool,
    pub custom_excludes: Vec<String>,
}

impl Default for CrawlerOptions {
    fn default() -> Self {
        Self {
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            honor_gitignore: true,
            custom_excludes: Vec::new(),
        }
    }
}

pub fn crawl_directory(root: &Path, options: &CrawlerOptions) -> Vec<CrawledFile> {
    let mut files = Vec::new();
    for entry in WalkBuilder::new(root)
        .git_ignore(options.honor_gitignore)
        .git_global(options.honor_gitignore)
        .git_exclude(options.honor_gitignore)
        .hidden(false)
        .follow_links(false)
        .build()
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }

        let is_denied = path.components().any(|c| {
            c.as_os_str().to_str().is_some_and(|name| {
                DENYLIST_DIRS.contains(&name)
                    || name.starts_with("cmake-build-")
                    || options.custom_excludes.iter().any(|ex| name.contains(ex))
            })
        });
        if is_denied {
            continue;
        }

        let lang = Lang::from_extension(path.extension().and_then(|e| e.to_str()).unwrap_or(""));
        if lang == Lang::Unknown {
            continue;
        }

        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if size == 0 || size > options.max_file_size || is_binary_file(path) {
            continue;
        }

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        files.push(CrawledFile {
            path: path.to_path_buf(),
            rel_path,
            lang,
            byte_size: size,
        });
    }

    files.sort_by(|a, b| a.rel_path.as_bytes().cmp(b.rel_path.as_bytes()));
    files
}

fn is_binary_file(path: &Path) -> bool {
    let mut buf = [0u8; 4096];
    File::open(path)
        .and_then(|mut f| f.read(&mut buf))
        .map(|n| buf[..n].contains(&0))
        .unwrap_or(false)
}

// --- TREE-SITTER INGEST ---

pub struct LanguageRegistry {
    languages: Vec<(Lang, Language, Query)>,
}

impl LanguageRegistry {
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

    pub fn get(&self, lang: Lang) -> Option<(&Language, &Query)> {
        self.languages
            .iter()
            .find(|(l, _, _)| *l == lang)
            .map(|(_, l, q)| (l, q))
    }
}

#[derive(Debug, Clone)]
struct RawDef {
    name: String,
    kind: SymKind,
    line: u32,
    start_byte: usize,
    end_byte: usize,
}

#[derive(Debug, Clone)]
struct RawRef {
    callee_name: String,
    line: u32,
    start_byte: usize,
    role: RefRole,
}

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

        let mut file_symbols = Vec::with_capacity(defs.len());
        for def in defs {
            let id = next_node_id;
            next_node_id += 1;
            let name = interner.intern(&def.name);
            let canon = interner.intern(&format!("{}::{}", file.rel_path, def.name));
            let mut sym = Symbol::new(id, name, canon, def.kind, file.lang, file_id, def.line);
            sym.sig_start_byte = def.start_byte as u32;
            sym.end_byte = def.end_byte as u32;
            file_symbols.push(sym);
        }

        for r in refs {
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
    fn test_core_functionality() {
        let interner = Interner::new();
        let s = interner.intern("foo");
        assert_eq!(interner.resolve(&s), "foo");

        assert_eq!(SymKind::Function.tag(), "fn");
        assert_eq!(RefRole::Call.tag(), "call");
        assert_eq!(Lang::Rust.tag(), "rs");
        assert!(Lang::Rust.is_compatible(Lang::Rust));
        assert!(Lang::C.is_compatible(Lang::Cpp));

        let exts = [
            ("rs", Lang::Rust),
            ("ts", Lang::TypeScript),
            ("js", Lang::JavaScript),
            ("py", Lang::Python),
            ("go", Lang::Go),
            ("c", Lang::C),
            ("cpp", Lang::Cpp),
            ("cu", Lang::Cuda),
            ("metal", Lang::Metal),
            ("java", Lang::Java),
            ("cs", Lang::CSharp),
            ("swift", Lang::Swift),
            ("sh", Lang::Bash),
            ("rb", Lang::Ruby),
            ("php", Lang::Php),
            ("lua", Lang::Lua),
            ("ex", Lang::Elixir),
            ("dart", Lang::Dart),
            ("kt", Lang::Kotlin),
            ("gd", Lang::GDScript),
            ("json", Lang::Json),
            ("toml", Lang::Toml),
            ("yaml", Lang::Yaml),
            ("md", Lang::Markdown),
            ("unknown", Lang::Unknown),
        ];
        for (ext, expected) in exts {
            assert_eq!(Lang::from_extension(ext), expected);
        }

        assert!(matches!(classify_capture("name"), Some(Capture::Name)));
        assert!(classify_capture("other").is_none());
        assert!(classify_capture("definition.function").is_some());
        assert!(classify_capture("reference.call").is_some());

        let reg = LanguageRegistry::new_tier1().unwrap();
        let (lang, query) = reg.get(Lang::Rust).unwrap();
        let res = parse_file_content(b"fn run() { log(); } fn log() {}", lang, query);
        assert_eq!(res.defs.len(), 2);
        assert_eq!(res.refs.len(), 1);

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join("test.rs"), "fn a() {}").unwrap();
        let crawled = crawl_directory(root, &CrawlerOptions::default());
        assert_eq!(crawled.len(), 1);
    }
}
