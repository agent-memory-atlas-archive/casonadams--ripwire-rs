use lasso::{Key, Spur, ThreadedRodeo};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 32-bit index into symbol tables. Not a pointer.
pub type NodeId = u32;

/// Sentinel value indicating no node (e.g., file-scope references).
pub const NO_NODE: NodeId = u32::MAX;
/// Thread-safe string interner backed by `lasso::ThreadedRodeo`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    #[must_use]
    pub fn all_strings(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.0.len());
        for i in 0..self.0.len() {
            if let Some(spur) = Spur::try_from_usize(i)
                && let Some(s) = self.0.try_resolve(&spur)
            {
                out.push(s.to_string());
            }
        }
        out
    }
    #[must_use]
    pub fn from_strings(strings: &[String]) -> Self {
        let rodeo = ThreadedRodeo::new();
        for s in strings {
            rodeo.get_or_intern(s);
        }
        Self(Arc::new(rodeo))
    }
}

impl std::ops::Index<Spur> for Interner {
    type Output = str;

    #[inline]
    fn index(&self, spur: Spur) -> &Self::Output {
        self.resolve(&spur)
    }
}

impl std::ops::Index<&Spur> for Interner {
    type Output = str;

    #[inline]
    fn index(&self, spur: &Spur) -> &Self::Output {
        self.resolve(spur)
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
    #[inline]
    #[must_use]
    pub const fn is_callable(self) -> bool {
        matches!(self, Self::Function | Self::Method)
    }

    #[inline]
    #[must_use]
    pub const fn is_type_container(self) -> bool {
        matches!(self, Self::Class | Self::Struct | Self::Interface)
    }

    #[inline]
    #[must_use]
    pub const fn is_code_symbol(self) -> bool {
        !matches!(self, Self::Section | Self::ModuleScope)
    }
}

impl std::fmt::Display for SymKind {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.tag())
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

impl std::fmt::Display for RefRole {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.tag())
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
    ObjC,
    Unknown,
}
impl Lang {
    #[inline]
    pub const fn tag(self) -> &'static str {
        const TAGS: [&str; 37] = [
            "rs", "ts", "js", "py", "go", "c", "cpp", "cuda", "metal", "java", "cs", "swift", "sh",
            "rb", "php", "lua", "ex", "dart", "kt", "gd", "json", "toml", "yaml", "md", "zig",
            "sol", "hcl", "scala", "erl", "html", "css", "sql", "proto", "gql", "docker", "m", "?",
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
            ("h", Lang::Cpp),
            ("cpp", Lang::Cpp),
            ("cc", Lang::Cpp),
            ("cxx", Lang::Cpp),
            ("hpp", Lang::Cpp),
            ("hxx", Lang::Cpp),
            ("hh", Lang::Cpp),
            ("cu", Lang::Cuda),
            ("cuh", Lang::Cuda),
            ("metal", Lang::Metal),
            ("m", Lang::ObjC),
            ("mm", Lang::ObjC),
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
        self == other || self.semantics().dialect_group() == other.semantics().dialect_group()
    }
    /// True if this language has syntax-backed dependency / import capability.
    #[must_use]
    pub fn is_dependency_capable(self) -> bool {
        self.semantics().is_dependency_capable()
    }
    /// True if both languages are dependency capable and share dialect compatibility.
    #[must_use]
    pub fn is_pair_dependency_capable(self, other: Self) -> bool {
        self.is_dependency_capable() && other.is_dependency_capable() && self.is_compatible(other)
    }
}

impl std::fmt::Display for Lang {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.tag())
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

    #[inline]
    #[must_use]
    pub const fn is_callable(&self) -> bool {
        self.kind.is_callable()
    }

    #[inline]
    #[must_use]
    pub const fn is_code_symbol(&self) -> bool {
        self.kind.is_code_symbol()
    }

    #[inline]
    #[must_use]
    pub const fn has_body(&self) -> bool {
        self.sig_end_byte < self.end_byte
    }

    #[inline]
    #[must_use]
    pub const fn end_line(&self) -> u32 {
        self.line.saturating_add(self.loc.saturating_sub(1))
    }

    /// Return the raw byte slice `[sig_start_byte..end_byte]` clamped to `file_bytes`.
    #[inline]
    #[must_use]
    pub fn source_bytes<'a>(&self, file_bytes: &'a [u8]) -> &'a [u8] {
        let start = (self.sig_start_byte as usize).min(file_bytes.len());
        let end = (self.end_byte as usize).min(file_bytes.len());
        if start < end {
            &file_bytes[start..end]
        } else {
            &[]
        }
    }

    /// Return the UTF-8 source text `[sig_start_byte..end_byte]` if non-empty.
    #[inline]
    #[must_use]
    pub fn source_text<'a>(&self, file_bytes: &'a [u8]) -> Option<std::borrow::Cow<'a, str>> {
        let slice = self.source_bytes(file_bytes);
        (!slice.is_empty()).then(|| String::from_utf8_lossy(slice))
    }

    /// Extract a single-line normalized declaration signature bounded before any body block.
    #[must_use]
    pub fn extract_signature(&self, file_bytes: &[u8], fallback_name: &str) -> String {
        let start = (self.sig_start_byte as usize).min(file_bytes.len());
        let end = (self.sig_end_byte as usize).min(file_bytes.len());
        if start >= end {
            return fallback_name.to_string();
        }
        let raw = String::from_utf8_lossy(&file_bytes[start..end]);
        let header = truncate_at_declaration_body(&raw);
        let collapsed = header.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            fallback_name.to_string()
        } else if collapsed.len() > 320 {
            let cut = collapsed.floor_char_boundary(320);
            format!("{}...", collapsed[..cut].trim_end())
        } else {
            collapsed
        }
    }
}

fn truncate_at_declaration_body(raw: &str) -> &str {
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    for (idx, b) in raw.bytes().enumerate() {
        match b {
            b'(' => paren_depth += 1,
            b')' => paren_depth = (paren_depth - 1).max(0),
            b'[' => bracket_depth += 1,
            b']' => bracket_depth = (bracket_depth - 1).max(0),
            b'{' | b';' if paren_depth == 0 && bracket_depth == 0 => {
                return raw[..idx].trim();
            }
            _ => {}
        }
    }
    raw.trim().trim_end_matches('{').trim()
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
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

    #[inline]
    #[must_use]
    pub fn sym(&self, id: NodeId) -> Option<&Symbol> {
        self.symbols.get(id as usize)
    }

    #[inline]
    #[must_use]
    pub fn file(&self, file_id: u32) -> Option<&FileInfo> {
        self.files.get(file_id as usize)
    }

    #[inline]
    #[must_use]
    pub fn file_path<'a>(&self, file_id: u32, interner: &'a Interner) -> &'a str {
        self.file(file_id)
            .and_then(|f| interner.try_resolve(&f.path))
            .unwrap_or("")
    }

    #[inline]
    #[must_use]
    pub fn sym_file_path<'a>(&self, sym: &Symbol, interner: &'a Interner) -> &'a str {
        self.file_path(sym.file_id, interner)
    }

    #[inline]
    #[must_use]
    pub fn sym_name<'a>(&self, sym: &Symbol, interner: &'a Interner) -> &'a str {
        interner.try_resolve(&sym.name).unwrap_or("")
    }
}

impl std::ops::Index<NodeId> for IngestResult {
    type Output = Symbol;

    #[inline]
    fn index(&self, id: NodeId) -> &Self::Output {
        &self.symbols[id as usize]
    }
}

/// Unified read-only view over an indexed corpus (`IngestResult` + `Interner`) to simplify caller queries.
pub trait CorpusView<'a> {
    fn ingest(&self) -> &'a IngestResult;
    fn interner(&self) -> &'a Interner;

    #[inline]
    #[must_use]
    fn sym_name(&self, sym: &Symbol) -> &'a str {
        self.interner().try_resolve(&sym.name).unwrap_or("")
    }

    #[inline]
    #[must_use]
    fn sym_scope(&self, sym: &Symbol) -> Option<&'a str> {
        sym.scope
            .as_ref()
            .and_then(|sc| self.interner().try_resolve(sc))
            .filter(|s| !s.is_empty())
    }

    #[inline]
    #[must_use]
    fn sym_path(&self, sym: &Symbol) -> &'a str {
        self.ingest().sym_file_path(sym, self.interner())
    }

    #[inline]
    #[must_use]
    fn file_path(&self, file_id: u32) -> &'a str {
        self.ingest().file_path(file_id, self.interner())
    }

    #[inline]
    #[must_use]
    fn is_test_symbol(&self, sym: &Symbol) -> bool {
        crate::gate::affected::is_test_or_bench_path(self.sym_path(sym))
            || self.sym_name(sym).starts_with("test_")
            || self
                .sym_scope(sym)
                .is_some_and(|sc| sc == "tests" || sc == "test" || sc.ends_with("Tests"))
    }

    #[inline]
    #[must_use]
    fn is_production_symbol(&self, sym: &Symbol) -> bool {
        sym.is_code_symbol() && !self.is_test_symbol(sym)
    }

    /// Collect interned callee names directly invoked inside test or benchmark files.
    #[must_use]
    fn test_called_symbol_names(&self) -> std::collections::HashSet<Spur> {
        self.ingest()
            .references
            .iter()
            .filter(|r| {
                matches!(r.role, RefRole::Call | RefRole::Macro)
                    && crate::gate::affected::is_test_or_bench_path(self.file_path(r.file_id))
            })
            .map(|r| r.callee_name)
            .collect()
    }

    /// Compute boolean bitmap of symbols transitively reached from any test symbol via `graph` call edges.
    #[must_use]
    fn compute_test_reachability(&self, graph: &crate::graph::Graph) -> Vec<bool> {
        let ingest = self.ingest();
        let n = ingest.symbols.len();
        let mut reached = vec![false; n];
        let mut queue = std::collections::VecDeque::new();
        let test_called_names = self.test_called_symbol_names();
        for sym in &ingest.symbols {
            let idx = sym.id as usize;
            if idx < n && (self.is_test_symbol(sym) || test_called_names.contains(&sym.name)) {
                reached[idx] = true;
                queue.push_back(sym.id);
            }
        }
        while let Some(curr) = queue.pop_front() {
            for (callee, _) in graph.callees(curr) {
                let idx = callee as usize;
                if idx < n && !reached[idx] {
                    reached[idx] = true;
                    queue.push_back(callee);
                }
            }
        }
        reached
    }

    /// Build a fast `path -> file_id` lookup map over all indexed files.
    #[must_use]
    fn file_index_map(&self) -> std::collections::HashMap<&'a str, u32> {
        self.ingest()
            .files
            .iter()
            .map(|f| (self.file_path(f.id), f.id))
            .collect()
    }

    /// Collect sorted, deduplicated `file:line` call-site strings from `caller_id` targeting any symbol in `callee_names`.
    #[must_use]
    fn caller_site_locations(&self, caller_id: NodeId, callee_names: &[Spur]) -> Vec<String> {
        let mut sites: Vec<(u32, u32)> = self
            .ingest()
            .references
            .iter()
            .filter(|r| {
                r.from_symbol == caller_id
                    && matches!(r.role, RefRole::Call | RefRole::Macro)
                    && callee_names.contains(&r.callee_name)
            })
            .map(|r| (r.file_id, r.line))
            .collect();
        sites.sort_unstable();
        sites.dedup();
        sites
            .into_iter()
            .map(|(fid, line)| format!("{}:{line}", self.file_path(fid)))
            .collect()
    }

    /// True if reference `r` targets `query` (either bare symbol/scope name or `Scope::member` / `Scope.member`).
    #[must_use]
    fn reference_matches_query(&self, r: &Reference, query: &str) -> bool {
        let interner = self.interner();
        if let Some((qual_part, leaf_part)) = split_scope_and_member(query) {
            if interner.resolve(&r.callee_name) != leaf_part {
                return false;
            }
            let qual_matches = r
                .qualifier
                .is_some_and(|q| interner.resolve(&q) == qual_part);
            let caller_sc_matches = self
                .ingest()
                .sym(r.from_symbol)
                .and_then(|s| s.scope)
                .is_some_and(|sc| interner.resolve(&sc) == qual_part);
            let unique_def_sc_matches = r.qualifier.is_none()
                && self
                    .ingest()
                    .symbols
                    .iter()
                    .filter(|s| s.name == r.callee_name)
                    .all(|s| s.scope.is_some_and(|sc| interner.resolve(&sc) == qual_part));
            return qual_matches || caller_sc_matches || unique_def_sc_matches;
        }
        interner.resolve(&r.callee_name) == query
            || r.qualifier.is_some_and(|q| interner.resolve(&q) == query)
    }

    /// Resolve sorted, deduplicated file paths that import any file defining `seed_ids` or reference the target symbols.
    #[must_use]
    fn resolve_symbol_importers(&self, seed_ids: &[NodeId]) -> Vec<&'a str> {
        let ingest = self.ingest();
        let seed_files: std::collections::HashSet<u32> = seed_ids
            .iter()
            .filter_map(|&id| ingest.sym(id).map(|s| s.file_id))
            .collect();
        let seed_names: std::collections::HashSet<Spur> = seed_ids
            .iter()
            .filter_map(|&id| ingest.sym(id).map(|s| s.name))
            .collect();
        let file_index = self.file_index_map();
        let mut importers = std::collections::BTreeSet::new();

        for r in &ingest.references {
            if seed_files.contains(&r.file_id) {
                continue;
            }
            let src_path = self.file_path(r.file_id);
            if !src_path.is_empty()
                && is_ref_importing_seed(
                    r,
                    src_path,
                    ingest,
                    self.interner(),
                    &file_index,
                    &seed_files,
                    &seed_names,
                )
            {
                importers.insert(src_path);
            }
        }
        importers.into_iter().collect()
    }
}

fn split_scope_and_member(query: &str) -> Option<(&str, &str)> {
    query
        .rsplit_once("::")
        .or_else(|| query.rsplit_once('.'))
        .filter(|(q, m)| !q.is_empty() && !m.is_empty())
}

fn is_ref_importing_seed(
    r: &Reference,
    src_path: &str,
    ingest: &IngestResult,
    interner: &Interner,
    file_index: &std::collections::HashMap<&str, u32>,
    seed_files: &std::collections::HashSet<u32>,
    seed_names: &std::collections::HashSet<Spur>,
) -> bool {
    if r.role != RefRole::Import {
        return seed_names.contains(&r.callee_name);
    }
    let raw = interner.resolve(&r.callee_name);
    let target_fid = ingest.file(r.file_id).and_then(|f| {
        f.lang
            .semantics()
            .resolve_import_target(src_path, raw, file_index)
    });
    if target_fid.is_some_and(|tf| seed_files.contains(&tf)) {
        return true;
    }
    seed_names.iter().any(|&sn| {
        let sname = interner.resolve(&sn);
        raw == sname || raw.ends_with(&format!("::{sname}"))
    })
}
impl<'a> CorpusView<'a> for (&'a IngestResult, &'a Interner) {
    #[inline]
    fn ingest(&self) -> &'a IngestResult {
        self.0
    }

    #[inline]
    fn interner(&self) -> &'a Interner {
        self.1
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
        assert_eq!(Lang::ObjC.tag(), "m");
        assert!(Lang::from_extension("rs") == Lang::Rust);
        assert!(Lang::from_extension("m") == Lang::ObjC);
        assert!(Lang::from_extension("mm") == Lang::ObjC);
        assert!(Lang::from_extension("unknown") == Lang::Unknown);
        assert!(Lang::C.is_compatible(Lang::ObjC));
        assert!(!Lang::Rust.is_compatible(Lang::Python));
    }

    #[test]
    fn test_interner() {
        let interner = Interner::new();
        let s1 = interner.intern("foo");
        let s2 = interner.intern("bar");
        assert_eq!(s1, interner.intern("foo"));
        assert_eq!(interner.resolve(&s1), "foo");
        assert_eq!(&interner[s1], "foo");
        assert_eq!(&interner[&s2], "bar");
    }

    #[test]
    fn test_corpus_view_and_symbol_predicates() {
        let interner = Interner::new();
        let mut ingest = IngestResult::new();
        ingest.files.push(FileInfo {
            id: 0,
            path: interner.intern("src/engine.rs"),
            lang: Lang::Rust,
            line_count: 20,
            byte_size: 200,
        });
        let mut sym = Symbol::new(
            0,
            interner.intern("execute_turn"),
            interner.intern("src/engine.rs::Engine::execute_turn"),
            SymKind::Method,
            Lang::Rust,
            0,
            10,
        )
        .with_scope(interner.intern("Engine"));
        sym.sig_start_byte = 10;
        sym.sig_end_byte = 30;
        sym.end_byte = 90;
        sym.loc = 5;
        ingest.symbols.push(sym);

        let view = (&ingest, &interner);
        assert!(sym.is_callable());
        assert!(sym.is_code_symbol());
        assert!(sym.has_body());
        assert_eq!(sym.end_line(), 14);
        assert_eq!(view.sym_name(&sym), "execute_turn");
        assert_eq!(view.sym_scope(&sym), Some("Engine"));
        assert_eq!(view.sym_path(&sym), "src/engine.rs");
        assert!(view.is_production_symbol(&sym));
        assert_eq!(Lang::from_extension("h"), Lang::Cpp);

        ingest.files.push(FileInfo {
            id: 1,
            path: interner.intern("src/main.rs"),
            lang: Lang::Rust,
            line_count: 15,
            byte_size: 120,
        });
        ingest.references.push(Reference::new(
            NO_NODE,
            interner.intern("crate::engine::Engine"),
            1,
            1,
            Lang::Rust,
            RefRole::Import,
        ));
        ingest.references.push(Reference::new(
            0,
            interner.intern("execute_turn"),
            1,
            8,
            Lang::Rust,
            RefRole::Call,
        ));
        let view = (&ingest, &interner);
        assert_eq!(view.resolve_symbol_importers(&[0]), vec!["src/main.rs"]);
        assert_eq!(
            view.caller_site_locations(0, &[interner.intern("execute_turn")]),
            vec!["src/main.rs:8".to_string()]
        );
        let qual_ref = Reference::new(0, interner.intern("lock"), 1, 9, Lang::Rust, RefRole::Call)
            .with_qualifier(interner.intern("TokenVault"));
        assert!(view.reference_matches_query(&qual_ref, "TokenVault"));
        assert!(view.reference_matches_query(&qual_ref, "TokenVault::lock"));
        assert!(view.reference_matches_query(&qual_ref, "TokenVault.lock"));
        assert!(!view.reference_matches_query(&qual_ref, "OtherVault::lock"));
    }

    #[test]
    fn test_symbol_extract_signature_and_source_text() {
        let interner = Interner::new();
        let mut sym = Symbol::new(
            0,
            interner.intern("resolveRefSpec"),
            interner.intern("src/q.h::resolveRefSpec"),
            SymKind::Function,
            Lang::Cpp,
            0,
            1,
        );
        let src = b"inline RefSpec resolveRefSpec(const std::string& root, int a[2]) {\n    return {};\n}";
        sym.sig_start_byte = 0;
        sym.sig_end_byte = src.len() as u32;
        sym.end_byte = src.len() as u32;
        assert_eq!(
            sym.extract_signature(src, "resolveRefSpec"),
            "inline RefSpec resolveRefSpec(const std::string& root, int a[2])"
        );
        assert!(sym.source_text(src).unwrap().contains("return {};"));
        assert_eq!(sym.extract_signature(b"", "fallback"), "fallback");
    }
}
