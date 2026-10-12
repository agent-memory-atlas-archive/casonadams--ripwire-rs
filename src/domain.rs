use lasso::{Spur, ThreadedRodeo};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 32-bit index into symbol tables. Not a pointer.
pub type NodeId = u32;

/// Sentinel value indicating no node (e.g., file-scope references).
pub const NO_NODE: NodeId = u32::MAX;
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
    pub fn get(&self, s: &str) -> Option<Spur> {
        self.0.get(s)
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
    Zig,
    Solidity,
    Hcl,
    Scala,
    Erlang,
    Html,
    Css,
    Sql,
    Proto,
    Graphql,
    Dockerfile,
    Unknown,
}
impl Lang {
    #[inline]
    pub const fn tag(self) -> &'static str {
        const TAGS: [&str; 36] = [
            "rs", "ts", "js", "py", "go", "c", "cpp", "cuda", "metal", "java", "cs", "swift", "sh",
            "rb", "php", "lua", "ex", "dart", "kt", "gd", "json", "toml", "yaml", "md", "zig",
            "sol", "hcl", "scala", "erl", "html", "css", "sql", "proto", "gql", "docker", "?",
        ];
        TAGS[self as usize]
    }
    /// Map file extensions to their corresponding language.
    pub fn from_extension(ext: &str) -> Self {
        const EXT_MAP: &[(&str, Lang)] = &[
            ("rs", Lang::Rust),
            ("ts", Lang::TypeScript),
            ("tsx", Lang::TypeScript),
            ("mts", Lang::TypeScript),
            ("cts", Lang::TypeScript),
            ("astro", Lang::TypeScript),
            ("js", Lang::JavaScript),
            ("jsx", Lang::JavaScript),
            ("mjs", Lang::JavaScript),
            ("cjs", Lang::JavaScript),
            ("py", Lang::Python),
            ("pyi", Lang::Python),
            ("go", Lang::Go),
            ("c", Lang::C),
            ("h", Lang::C),
            ("cpp", Lang::Cpp),
            ("cc", Lang::Cpp),
            ("cxx", Lang::Cpp),
            ("hpp", Lang::Cpp),
            ("hxx", Lang::Cpp),
            ("hh", Lang::Cpp),
            ("cu", Lang::Cuda),
            ("cuh", Lang::Cuda),
            ("metal", Lang::Metal),
            ("java", Lang::Java),
            ("cs", Lang::CSharp),
            ("swift", Lang::Swift),
            ("sh", Lang::Bash),
            ("bash", Lang::Bash),
            ("zsh", Lang::Bash),
            ("rb", Lang::Ruby),
            ("rake", Lang::Ruby),
            ("jbuilder", Lang::Ruby),
            ("php", Lang::Php),
            ("phtml", Lang::Php),
            ("lua", Lang::Lua),
            ("ex", Lang::Elixir),
            ("exs", Lang::Elixir),
            ("dart", Lang::Dart),
            ("kt", Lang::Kotlin),
            ("kts", Lang::Kotlin),
            ("gd", Lang::GDScript),
            ("json", Lang::Json),
            ("toml", Lang::Toml),
            ("yml", Lang::Yaml),
            ("yaml", Lang::Yaml),
            ("md", Lang::Markdown),
            ("markdown", Lang::Markdown),
            ("rst", Lang::Markdown),
            ("adoc", Lang::Markdown),
            ("org", Lang::Markdown),
            ("mdx", Lang::Markdown),
            ("zig", Lang::Zig),
            ("zon", Lang::Zig),
            ("sol", Lang::Solidity),
            ("tf", Lang::Hcl),
            ("hcl", Lang::Hcl),
            ("tfvars", Lang::Hcl),
            ("scala", Lang::Scala),
            ("sc", Lang::Scala),
            ("erl", Lang::Erlang),
            ("hrl", Lang::Erlang),
            ("html", Lang::Html),
            ("htm", Lang::Html),
            ("css", Lang::Css),
            ("scss", Lang::Css),
            ("sass", Lang::Css),
            ("less", Lang::Css),
            ("sql", Lang::Sql),
            ("proto", Lang::Proto),
            ("graphql", Lang::Graphql),
            ("gql", Lang::Graphql),
            ("dockerfile", Lang::Dockerfile),
            ("containerfile", Lang::Dockerfile),
        ];

        for &(e, l) in EXT_MAP {
            if ext.eq_ignore_ascii_case(e) {
                return l;
            }
        }
        Self::Unknown
    }
    /// True if these two languages are compatible for cross-symbol resolution.
    pub fn is_compatible(self, other: Self) -> bool {
        if self == other {
            return true;
        }
        let is_c_family = |l| matches!(l, Self::C | Self::Cpp | Self::Cuda | Self::Metal);
        if is_c_family(self) && is_c_family(other) {
            return true;
        }
        matches!(
            (self, other),
            (Self::TypeScript, Self::JavaScript)
                | (Self::JavaScript, Self::TypeScript)
                | (Self::Java, Self::Kotlin)
                | (Self::Kotlin, Self::Java)
                | (Self::Java, Self::Scala)
                | (Self::Scala, Self::Java)
                | (Self::Elixir, Self::Erlang)
                | (Self::Erlang, Self::Elixir)
                | (Self::Html, Self::Css)
                | (Self::Css, Self::Html)
        )
    }
    /// True if this language has syntax-backed dependency / import capability.
    #[must_use]
    pub fn is_dependency_capable(self) -> bool {
        matches!(
            self,
            Self::Rust
                | Self::TypeScript
                | Self::JavaScript
                | Self::Python
                | Self::Go
                | Self::C
                | Self::Cpp
                | Self::Cuda
                | Self::Metal
                | Self::Java
                | Self::CSharp
                | Self::Swift
                | Self::Bash
                | Self::Ruby
                | Self::Php
                | Self::Lua
                | Self::Elixir
                | Self::Dart
                | Self::Kotlin
                | Self::Zig
                | Self::Solidity
                | Self::Hcl
                | Self::Scala
                | Self::Erlang
                | Self::Html
                | Self::Css
                | Self::Sql
                | Self::Proto
                | Self::Graphql
        )
    }
    /// True if both languages are dependency capable and share dialect compatibility.
    #[must_use]
    pub fn is_pair_dependency_capable(self, other: Self) -> bool {
        self.is_dependency_capable() && other.is_dependency_capable() && self.is_compatible(other)
    }
}

/// Indexed definition in the symbol table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    pub id: NodeId,
    pub name: Spur,
    pub canonical_id: Spur,
    pub scope: Option<Spur>,
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
    pub humps: u16,
    pub deep: u16,
    pub ev: u16,
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
            scope: None,
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
            humps: 0,
            deep: 0,
            ev: 1,
        }
    }

    #[must_use]
    pub fn with_scope(mut self, scope: Spur) -> Self {
        self.scope = Some(scope);
        self
    }
}

/// Unresolved reference extracted from AST syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub from_symbol: NodeId,
    pub callee_name: Spur,
    pub qualifier: Option<Spur>,
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
            qualifier: None,
            file_id,
            line,
            start_byte: 0,
            lang,
            role,
            arg_count: 0,
            arg_count_known: false,
        }
    }

    #[must_use]
    pub fn with_qualifier(mut self, qualifier: Spur) -> Self {
        self.qualifier = Some(qualifier);
        self
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
    fn test_domain_tags() {
        assert_eq!(SymKind::Function.tag(), "fn");
        assert_eq!(RefRole::Call.tag(), "call");
        assert_eq!(Lang::Rust.tag(), "rs");
        assert_eq!(Lang::from_extension("rs"), Lang::Rust);
        assert_eq!(Lang::from_extension("unknown"), Lang::Unknown);
        assert!(Lang::Rust.is_compatible(Lang::Rust));
        assert!(Lang::C.is_compatible(Lang::Cpp));
        assert!(!Lang::Rust.is_compatible(Lang::Python));
    }

    #[test]
    fn test_interner() {
        let interner = Interner::new();
        let s1 = interner.intern("foo");
        let s2 = interner.intern("bar");
        assert_eq!(s1, interner.intern("foo"));
        assert_eq!(interner.resolve(&s1), "foo");
        assert_eq!(interner.resolve(&s2), "bar");
    }
}
