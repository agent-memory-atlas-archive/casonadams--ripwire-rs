use lasso::Spur;
use serde::{Deserialize, Serialize};

/// 32-bit index into symbol tables. Not a pointer.
pub type NodeId = u32;

/// Sentinel value indicating no node (e.g., file-scope references).
pub const NO_NODE: NodeId = u32::MAX;

/// Symbol kind corresponding to the terse XML attribute `t="..."`.
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

    /// Map file extensions to their corresponding language.
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

    /// True if these two languages are compatible for cross-symbol resolution.
    pub fn is_compatible(self, other: Self) -> bool {
        if self == other {
            return true;
        }
        matches!(
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

/// Indexed definition in the symbol table.
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

/// Unresolved reference extracted from AST syntax.
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

/// Metadata for a crawled file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: u32,
    pub path: Spur,
    pub lang: Lang,
    pub line_count: u32,
    pub byte_size: u64,
}

/// Ingest output: the contract between AST ingestion and graph construction.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sym_kind_tags() {
        assert_eq!(SymKind::Function.tag(), "fn");
        assert_eq!(SymKind::Method.tag(), "method");
        assert_eq!(SymKind::Class.tag(), "cls");
        assert_eq!(SymKind::Struct.tag(), "struct");
        assert_eq!(SymKind::Interface.tag(), "iface");
        assert_eq!(SymKind::Var.tag(), "var");
        assert_eq!(SymKind::Section.tag(), "sec");
        assert_eq!(SymKind::Macro.tag(), "macro");
        assert_eq!(SymKind::Field.tag(), "field");
        assert_eq!(SymKind::Other.tag(), "other");
        assert_eq!(SymKind::ModuleScope.tag(), "modscope");
    }

    #[test]
    fn test_ref_role_tags() {
        assert_eq!(RefRole::Call.tag(), "call");
        assert_eq!(RefRole::Read.tag(), "read");
        assert_eq!(RefRole::Write.tag(), "write");
        assert_eq!(RefRole::Import.tag(), "import");
        assert_eq!(RefRole::Extends.tag(), "extends");
        assert_eq!(RefRole::Macro.tag(), "macro");
        assert_eq!(RefRole::Type.tag(), "type");
        assert_eq!(RefRole::Value.tag(), "value");
        assert_eq!(RefRole::Through.tag(), "through");
    }

    #[test]
    fn test_lang_extension_mapping() {
        assert_eq!(Lang::from_extension("rs"), Lang::Rust);
        assert_eq!(Lang::from_extension("ts"), Lang::TypeScript);
        assert_eq!(Lang::from_extension("js"), Lang::JavaScript);
        assert_eq!(Lang::from_extension("py"), Lang::Python);
        assert_eq!(Lang::from_extension("go"), Lang::Go);
        assert_eq!(Lang::from_extension("cpp"), Lang::Cpp);
        assert_eq!(Lang::from_extension("unknown_xyz"), Lang::Unknown);
    }

    #[test]
    fn test_lang_compatibility() {
        assert!(Lang::Rust.is_compatible(Lang::Rust));
        assert!(Lang::C.is_compatible(Lang::Cpp));
        assert!(Lang::Cpp.is_compatible(Lang::C));
        assert!(Lang::TypeScript.is_compatible(Lang::JavaScript));
        assert!(!Lang::Rust.is_compatible(Lang::Python));
    }
}
