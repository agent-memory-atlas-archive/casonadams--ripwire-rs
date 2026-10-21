macro_rules! impl_grammar_method {
    (crate $lang_expr:expr, $scm:expr) => {
        fn grammar_and_query(
            &self,
        ) -> Option<(&'static tree_sitter::Language, &'static tree_sitter::Query)> {
            static ENTRY: std::sync::LazyLock<(tree_sitter::Language, tree_sitter::Query)> =
                std::sync::LazyLock::new(|| {
                    let ts: tree_sitter::Language = $lang_expr.into();
                    let q = tree_sitter::Query::new(&ts, include_str!($scm)).expect("valid query");
                    (ts, q)
                });
            Some((&ENTRY.0, &ENTRY.1))
        }
    };
    (ffi $ffi_fn:ident, $scm:expr) => {
        fn grammar_and_query(
            &self,
        ) -> Option<(&'static tree_sitter::Language, &'static tree_sitter::Query)> {
            unsafe extern "C" {
                fn $ffi_fn() -> *const tree_sitter::ffi::TSLanguage;
            }
            static ENTRY: std::sync::LazyLock<(tree_sitter::Language, tree_sitter::Query)> =
                std::sync::LazyLock::new(|| {
                    let ts = unsafe { tree_sitter::Language::from_raw($ffi_fn()) };
                    let q = tree_sitter::Query::new(&ts, $scm).expect("valid query");
                    (ts, q)
                });
            Some((&ENTRY.0, &ENTRY.1))
        }
    };
}
pub(crate) use impl_grammar_method;

mod complexity_kinds;
mod foreign;
mod syntax_kinds;
pub use complexity_kinds::ComplexityKinds;
pub use foreign::ForeignExport;
pub use syntax_kinds::SyntaxKinds;
pub mod bash;
pub mod brightscript;
pub mod c;
pub mod cobol;
pub mod cpp;
pub mod csharp;
pub mod cuda;
pub mod dart;
pub mod elixir;
pub mod erlang;
pub mod fortran;
pub mod gdscript;
pub mod go;
pub mod graphql;
pub mod hcl;
pub mod java;
pub mod javascript;
pub mod json;
pub mod kotlin;
pub(crate) mod lint;
pub(crate) mod lint_hooks;
pub mod lua;
pub mod markdown;
pub mod metal;
pub mod objc;
pub mod php;
pub mod proto;
pub mod python;
pub mod reads;
pub mod ruby;
pub mod rust;
pub mod scala;
pub mod solidity;
pub mod source;
pub mod sql;
pub mod swift;
pub mod toml;
pub mod typescript;
pub mod yaml;
pub mod zig;

use crate::domain::{Lang, SymKind};
use crate::ingest::FileParseResult;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Language, Query};

/// Cross-language symbol resolution compatibility group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialectGroup {
    CFamily,
    Web,
    Jvm,
    Beam,
    Markup,
    Isolated(Lang),
}

/// How a language determines whether a symbol declaration is public.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityMode {
    /// Requires explicit `pub` or `public` modifier on the AST node (Rust, Java, C#, Kotlin, PHP, Swift).
    ExplicitModifier,
    /// Determined by symbol name convention (Go uppercase initial, Python/Dart/Ruby/Elixir/Lua/Bash/GDScript leading `_`).
    ByNameConvention,
    /// Public unless leading `_` on identifier.
    DefaultPublic,
}

/// Context passed to `LanguageSemantics::is_def_capture_allowed` to filter raw query captures.
#[derive(Clone, Copy)]
pub struct DefCaptureGateContext<'a> {
    pub kind: SymKind,
    pub sub: Option<&'static str>,
    pub name: &'a str,
    pub name_node: Option<tree_sitter::Node<'a>>,
    pub def_node: Option<tree_sitter::Node<'a>>,
    pub source_bytes: &'a [u8],
}

/// Unified trait encapsulating all language-specific syntax, AST, visibility, and resolution rules.
pub trait LanguageSemantics: Sync {
    /// Short canonical tag used in XML output (`"rs"`, `"ts"`, etc.).
    fn tag(&self) -> &'static str;

    /// Grammar node kinds the language-neutral extraction, slicing, and contract passes use.
    fn syntax_kinds(&self) -> &'static SyntaxKinds;

    /// Receiver of a member access whose grammar keeps it outside the access node (sibling selectors).
    fn member_receiver<'a>(&self, _access: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
        None
    }

    /// Receiver spellings that name the enclosing type's own instance.
    fn self_receivers(&self) -> &'static [&'static str] {
        &["self", "this", "Self"]
    }

    /// Definitions in this file that a binding framework exposes to another language.
    fn foreign_exports(&self, _root: tree_sitter::Node<'_>, _source: &[u8]) -> Vec<ForeignExport> {
        Vec::new()
    }

    /// File extensions mapped to this language (defaults to `&[self.tag()]` when empty).
    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    /// True if `ext` (case-insensitively) maps to this language.
    fn matches_extension(&self, ext: &str) -> bool {
        let exts = self.extensions();
        if exts.is_empty() {
            self.tag() != "?" && ext.eq_ignore_ascii_case(self.tag())
        } else {
            exts.iter().any(|e| ext.eq_ignore_ascii_case(e))
        }
    }

    /// Compiled Tree-sitter grammar and tags query for this language.
    fn grammar_and_query(&self) -> Option<(&'static Language, &'static Query)> {
        None
    }

    /// Grammar node kinds this language contributes to `cx` / `ccx` complexity.
    fn complexity_kinds(&self) -> &'static ComplexityKinds;

    /// A call qualifier the generic receiver extraction cannot see (e.g. Dart's sibling selector chains).
    fn call_qualifier(
        &self,
        _name_node: tree_sitter::Node<'_>,
        _source_bytes: &[u8],
    ) -> Option<String> {
        None
    }

    /// True if a type-qualified call whose qualifier names no indexed definition is external, not a
    /// candidate for same-named members of unrelated types.
    fn qualifier_must_be_indexed(&self) -> bool {
        false
    }

    /// True if `'` can introduce a lifetime identifier (`'a`) rather than a character literal.
    fn has_lifetime_tick_syntax(&self) -> bool {
        false
    }

    fn source_region(&self, kind: &str) -> Option<source::SourceRegion> {
        source::node_region(kind)
    }
    /// Visualization fill color for HTML call-graph export.
    fn display_color(&self) -> &'static str {
        "#888888"
    }

    /// Cross-language resolution dialect group.
    fn dialect_group(&self) -> DialectGroup;

    /// True if syntax-backed import/include resolution is supported for dependency metrics.
    fn is_dependency_capable(&self) -> bool {
        true
    }

    /// True if `#` introduces a line comment in token stream clone detection.
    fn uses_hash_comments(&self) -> bool {
        false
    }

    /// True if this language's tags query captures read sites, so `--uses` can list them.
    fn indexes_reads(&self) -> bool {
        self.grammar_and_query().is_some_and(|(_, q)| {
            q.capture_names()
                .iter()
                .any(|c| matches!(*c, "reference.read" | "reference.attribute"))
        })
    }

    /// Reject declaration targets, labels, types and callees from broad read captures.
    fn is_read_capture_allowed(&self, _node: tree_sitter::Node<'_>) -> bool {
        true
    }

    /// Lexical locals must not resolve to unrelated same-named repository definitions.
    fn read_bindings<'a>(
        &self,
        _root: tree_sitter::Node<'_>,
        _source: &'a [u8],
    ) -> reads::ReadBindings<'a> {
        reads::ReadBindings::default()
    }

    /// True if `--layout` C-family byte-layout modeling is supported.
    fn supports_c_layout(&self) -> bool {
        false
    }

    /// True if `--slice` intra-procedural def-use slicing is supported.
    fn supports_slice(&self) -> bool {
        true
    }

    /// True if string literals in this language should be scanned for embedded SQL DML/DDL table references.
    fn supports_embedded_sql(&self) -> bool {
        true
    }

    /// True if files of this language are prose documents (excluded from code-identifier sets).
    fn is_prose_document(&self) -> bool {
        false
    }

    /// True if files of this language are structured data (JSON, TOML, YAML), excluded from code inventory.
    fn is_data_format(&self) -> bool {
        false
    }

    /// True if `path` is a public interface header (`.h`, `.hpp`, `.hh`, `.hxx`, `.cuh`) in a C-family language.
    fn is_header_file(&self, path: &str) -> bool {
        self.dialect_group() == DialectGroup::CFamily
            && matches!(
                Path::new(path).extension().and_then(|e| e.to_str()),
                Some("h" | "hpp" | "hh" | "hxx" | "cuh")
            )
    }

    /// True if definitions in this language represent SQL tables targeted by `RefRole::Read` / `RefRole::Write`.
    fn is_table_read_write_target(&self) -> bool {
        false
    }

    /// How visibility is determined when comparing HEAD vs working-tree contracts.
    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::DefaultPublic
    }

    /// Maximum byte size allowed for indexing a file of this language.
    fn max_file_size(&self, default_max: u64) -> u64 {
        default_max
    }

    /// Refine file dialect from source content when the extension is shared (e.g., `.h` across C, C++, and Objective-C).
    fn refine_dialect(&self, base_lang: Lang, _rel_path: &str, _source_bytes: &[u8]) -> Lang {
        base_lang
    }

    /// True if a symbol represents a conventional program entrypoint (`main`, `__main__.py`, etc.) for `<inv><entry>`.
    fn is_entrypoint_symbol(
        &self,
        sym_name: &str,
        scope: Option<&str>,
        kind: SymKind,
        rel_path: &str,
    ) -> bool {
        if !kind.is_callable() || is_non_entrypoint_path(rel_path) {
            return false;
        }
        sym_name == "main" && scope.is_none()
    }

    /// Optional custom file parser override (used by Markdown, YAML nesting guard, and Astro frontmatter).
    fn parse_file_override(
        &self,
        _source_bytes: &[u8],
        _rel_path: &str,
        _language: &Language,
        _query: &Query,
    ) -> Option<FileParseResult> {
        None
    }

    /// Resolve the AST node containing the definition body and its ending byte offset.
    fn resolve_def_body_and_end<'a>(
        &self,
        def_node: Option<tree_sitter::Node<'a>>,
        default_end: usize,
    ) -> (Option<tree_sitter::Node<'a>>, usize) {
        let Some(dn) = def_node else {
            return (None, default_end);
        };
        if let Some(parent) = find_enclosing_definition_node(dn) {
            return (Some(parent), parent.end_byte());
        }
        (Some(dn), default_end)
    }

    /// Extract parameter count and format canonical symbol name (e.g., Elixir `name/arity`).
    fn resolve_def_params_and_name(
        &self,
        def_node: Option<tree_sitter::Node<'_>>,
        _source_bytes: &[u8],
        _kind: SymKind,
        orig_name: &str,
    ) -> (u16, String) {
        let params = def_node
            .map(|n| crate::gate::contract::extract_param_count(n, self.syntax_kinds()))
            .unwrap_or(0);
        (params, orig_name.to_string())
    }

    /// Format a call reference's target name (e.g., Elixir `name/arity`).
    fn format_callee_name(&self, name: &str, _arg_count: u16, _arg_count_known: bool) -> String {
        name.to_string()
    }

    /// Optional language-specific enclosing scope extractor for non-standard container AST shapes.
    fn extract_custom_scope(
        &self,
        _parent: tree_sitter::Node<'_>,
        _name_node: tree_sitter::Node<'_>,
        _source_bytes: &[u8],
    ) -> Option<String> {
        None
    }

    /// True if a captured identifier is a language keyword or cast operator that must not be indexed.
    fn is_builtin_keyword(&self, _name: &str) -> bool {
        false
    }

    /// True if a captured call reference is a language builtin/macro that should not form a user call edge.
    fn should_ignore_call(&self, _name_node: Option<tree_sitter::Node<'_>>, name: &str) -> bool {
        self.is_builtin_keyword(name)
    }

    /// Standard library / builtin member names declined on untyped receivers.
    fn std_member_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Standard library / runtime namespace qualifiers classified as `External`.
    fn std_qualifier_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// True if a method name belongs to the language's standard container/string/builtin types (declined on untyped receivers).
    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        self.std_member_names().contains(&name)
    }

    /// True if an explicit call qualifier names a standard library or builtin runtime namespace/type.
    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        self.std_qualifier_names().contains(&qualifier)
    }

    /// Optional byte-preserving macro-blanking pass invoked only when the primary tree-sitter parse contains syntax errors.
    fn blank_error_macros(&self, _source_bytes: &[u8]) -> Option<Vec<u8>> {
        None
    }

    /// True if a captured definition satisfies language-specific structural/naming gates.
    fn is_def_capture_allowed(&self, _ctx: &DefCaptureGateContext<'_>) -> bool {
        true
    }
    /// True if a symbol name is public by language naming convention.
    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_')
    }

    /// Subject a camelCase test file (`WidgetTest`) under a `test`/`tests` dir names, for languages with that convention.
    fn camel_test_partner_subject<'a>(&self, _file_stem: &'a str) -> Option<&'a str> {
        None
    }

    /// File stems that hold only tests by this language's module convention (e.g. Rust `tests.rs`).
    fn is_test_module_stem(&self, _file_stem: &str) -> bool {
        false
    }

    /// True for shell scripts: test files that run programs as subprocesses the call graph cannot see.
    fn is_script_runner(&self) -> bool {
        false
    }

    /// Multiplier for this language's symbols in task retrieval; below 1 for declaration-only formats.
    fn retrieval_weight(&self) -> f64 {
        1.0
    }

    /// File names this language's code generators emit by convention (e.g. Go `x.pb.go`); never indexed.
    fn is_generated_file_name(&self, _file_name: &str) -> bool {
        false
    }

    /// Manifest whose nearest ancestor copy selects this language's test runner (e.g. `package.json`).
    fn test_runner_manifest(&self) -> Option<&'static str> {
        None
    }

    /// Command that runs the test file at repo-relative `path`, from its source and nearest manifest text.
    fn test_run_command(
        &self,
        _path: &str,
        _source: &str,
        _manifest: Option<&str>,
    ) -> Option<String> {
        None
    }

    /// Terminator of a literal (quoted) heredoc that `line` opens; the body lines until it are data, not code.
    fn quoted_heredoc_terminator<'a>(&self, _line: &'a str) -> Option<&'a str> {
        None
    }

    /// True if an AST definition node has public visibility in this language.
    fn check_node_visibility(&self, def_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
        match self.visibility_mode() {
            VisibilityMode::ExplicitModifier => has_any_visibility_modifier(def_node, source_bytes),
            VisibilityMode::ByNameConvention => {
                check_leading_underscore_private(def_node, source_bytes, self.syntax_kinds())
            }
            VisibilityMode::DefaultPublic => true,
        }
    }

    /// Import specifier implied by a module declaration symbol (e.g. Rust `mod foo;` -> `self::foo`).
    fn symbol_implied_import(&self, _kind: SymKind, _name: &str) -> Option<String> {
        None
    }
    /// The module an import specifier names and the identifier it binds in the importing file, when
    /// calls qualified by that identifier resolve into the module (Python `import x as y` -> `("x", "y")`).
    fn import_binding<'a>(&self, _raw_target: &'a str) -> Option<(&'a str, &'a str)> {
        None
    }
    /// False where `name(...)` always calls a function even when a parameter shares the name (Ruby).
    fn parameters_shadow_calls(&self) -> bool {
        true
    }
    /// Project manifests whose module-path aliases apply to this language (e.g. `tsconfig.json`).
    fn import_alias_manifests(&self) -> &'static [&'static str] {
        &[]
    }
    /// `(pattern, target)` aliases a manifest declares, `target` relative to the manifest's directory.
    fn parse_import_aliases(&self, _content: &str) -> Vec<(String, String)> {
        Vec::new()
    }
    /// Resolve an import/include specifier from `importer_path` to a target `file_id` in `file_index`.
    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        let clean = strip_import_quotes(raw_target);
        if clean.is_empty() {
            return None;
        }
        resolve_relative_or_exact(importer_path, clean, file_index, &[])
    }

    /// Tag this language carries in the `--lint-catalog` `lang=` lists.
    fn lint_tag(&self) -> &'static str {
        self.tag()
    }

    /// Path-sensitive grammar shared by source analyses.
    fn source_grammar(&self, _path: &str) -> Option<&'static tree_sitter::Language> {
        self.grammar_and_query().map(|(language, _)| language)
    }

    /// Catalog rules that also fire on this language although `lang=` does not list it.
    fn lint_extra_rules(&self) -> &'static [&'static str] {
        &[]
    }

    fn lint_name_scope<'a>(&self, _scope: Option<&'a str>) -> Option<&'a str> {
        None
    }

    fn is_reserved_identifier(&self, name: &str) -> bool {
        self.dialect_group() == DialectGroup::CFamily
            && (name.starts_with("__")
                || name
                    .strip_prefix('_')
                    .is_some_and(|rest| rest.starts_with(char::is_uppercase)))
    }

    /// Declared return type in a callable's signature text, when the grammar names one.
    fn lint_return_type<'a>(&self, _sig: &'a str, _name: &str) -> Option<&'a str> {
        None
    }

    /// Spellings of the boolean type a predicate-named callable may return.
    fn lint_bool_types(&self) -> &'static [&'static str] {
        &["bool", "boolean", "Bool", "Boolean", "_Bool"]
    }

    fn lint_void_types(&self) -> &'static [&'static str] {
        &["void", "Void", "None"]
    }

    /// Leading keywords of a declaration that defines a type instead of a variable.
    fn type_declaration_keywords(&self) -> &'static [&'static str] {
        &[]
    }

    /// The expression a self-assignment compares against its target: grammars that flatten
    /// selector chains (`x = x.f()`) parse only the leading identifier as the right-hand value.
    fn assigned_value_root<'a>(&self, rhs: &'a str) -> &'a str {
        rhs
    }

    fn lint_constant_initializer(&self, node: tree_sitter::Node<'_>, source: &str) -> bool {
        lint_hooks::constant_initializer(node, source)
    }

    /// True when the cache-layout lint family applies (template containers, loop bodies).
    fn supports_cache_lint(&self) -> bool {
        false
    }
}

pub struct UnknownLanguage;
pub static UNKNOWN_LANG: UnknownLanguage = UnknownLanguage;

impl LanguageSemantics for UnknownLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &ComplexityKinds::NONE
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SyntaxKinds::NONE
    }

    fn tag(&self) -> &'static str {
        "?"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Unknown)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }
}

pub const ALL_LANGS: [Lang; 36] = [
    Lang::Rust,
    Lang::TypeScript,
    Lang::JavaScript,
    Lang::Python,
    Lang::Go,
    Lang::C,
    Lang::Cpp,
    Lang::Cuda,
    Lang::Metal,
    Lang::Java,
    Lang::CSharp,
    Lang::Swift,
    Lang::Bash,
    Lang::Ruby,
    Lang::Php,
    Lang::Lua,
    Lang::Elixir,
    Lang::Dart,
    Lang::Kotlin,
    Lang::GDScript,
    Lang::Json,
    Lang::Toml,
    Lang::Yaml,
    Lang::Markdown,
    Lang::Zig,
    Lang::Solidity,
    Lang::Hcl,
    Lang::Scala,
    Lang::Erlang,
    Lang::Sql,
    Lang::Proto,
    Lang::Graphql,
    Lang::ObjC,
    Lang::BrightScript,
    Lang::Fortran,
    Lang::Cobol,
];

static SEMANTICS_TABLE: [&dyn LanguageSemantics; 37] = [
    &rust::RUST_LANG,
    &typescript::TYPESCRIPT_LANG,
    &javascript::JAVASCRIPT_LANG,
    &python::PYTHON_LANG,
    &go::GO_LANG,
    &c::C_LANG,
    &cpp::CPP_LANG,
    &cuda::CUDA_LANG,
    &metal::METAL_LANG,
    &java::JAVA_LANG,
    &csharp::CSHARP_LANG,
    &swift::SWIFT_LANG,
    &bash::BASH_LANG,
    &ruby::RUBY_LANG,
    &php::PHP_LANG,
    &lua::LUA_LANG,
    &elixir::ELIXIR_LANG,
    &dart::DART_LANG,
    &kotlin::KOTLIN_LANG,
    &gdscript::GDSCRIPT_LANG,
    &json::JSON_LANG,
    &toml::TOML_LANG,
    &yaml::YAML_LANG,
    &markdown::MARKDOWN_LANG,
    &zig::ZIG_LANG,
    &solidity::SOLIDITY_LANG,
    &hcl::HCL_LANG,
    &scala::SCALA_LANG,
    &erlang::ERLANG_LANG,
    &sql::SQL_LANG,
    &proto::PROTO_LANG,
    &graphql::GRAPHQL_LANG,
    &objc::OBJC_LANG,
    &brightscript::BRIGHTSCRIPT_LANG,
    &fortran::FORTRAN_LANG,
    &cobol::COBOL_LANG,
    &UNKNOWN_LANG,
];

impl Lang {
    /// Return the static `LanguageSemantics` implementation for this language.
    #[inline]
    #[must_use]
    pub fn semantics(self) -> &'static dyn LanguageSemantics {
        SEMANTICS_TABLE[self as usize]
    }
}

pub(crate) fn is_non_entrypoint_path(rel_path: &str) -> bool {
    if crate::gate::affected::is_test_or_bench_path(rel_path) {
        return true;
    }
    Path::new(rel_path).components().any(|c| {
        c.as_os_str().to_str().is_some_and(|s| {
            matches!(
                s.to_ascii_lowercase().as_str(),
                "example" | "examples" | "doc" | "docs"
            )
        })
    })
}

pub(crate) fn find_enclosing_definition_node(
    node: tree_sitter::Node<'_>,
) -> Option<tree_sitter::Node<'_>> {
    let mut curr = node;
    while let Some(parent) = curr.parent() {
        if matches!(
            parent.kind(),
            "function_definition"
                | "method_definition"
                | "constructor_definition"
                | "destructor_definition"
        ) {
            return Some(parent);
        }
        if matches!(
            parent.kind(),
            "compound_statement"
                | "statement_block"
                | "block"
                | "declaration_list"
                | "field_declaration_list"
                | "class_body"
                | "ERROR"
        ) {
            return None;
        }
        curr = parent;
    }
    None
}

pub(crate) fn check_leading_underscore_private(
    def_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
    kinds: &SyntaxKinds,
) -> bool {
    let node = crate::gate::contract::unwrap_signature_node(def_node, kinds);
    let is_private = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source_bytes).ok())
        .is_some_and(|s| s.starts_with('_'));
    !is_private
}

fn has_visibility_modifier(child: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
    let kind = child.kind();
    if (kind == "visibility_modifier" || kind == "accessibility_modifier")
        && let Ok(text) = child.utf8_text(source_bytes)
    {
        return text.contains("pub") || text.contains("public");
    }
    false
}

pub(crate) fn has_any_visibility_modifier(
    def_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> bool {
    for i in 0..def_node.child_count() {
        if let Some(child) = def_node.child(i)
            && has_visibility_modifier(child, source_bytes)
        {
            return true;
        }
    }
    false
}

#[must_use]
pub fn strip_import_quotes(raw: &str) -> &str {
    raw.trim()
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | ';'))
        .trim()
}

/// JS-family runner: the nearest manifest's vitest/jest, else `require('node:test')` when `node_test` applies.
#[must_use]
pub(crate) fn js_test_run_command(
    path: &str,
    source: &str,
    manifest: Option<&str>,
    node_test: bool,
) -> Option<String> {
    match manifest {
        Some(m) if m.contains("vitest") => Some(format!("npx vitest run {path}")),
        Some(m) if m.contains("jest") => Some(format!("npx jest {path}")),
        _ => (node_test
            && (source.contains("require('node:test')")
                || source.contains("require(\"node:test\")")))
        .then(|| format!("node --test {path}")),
    }
}

#[must_use]
pub(crate) fn strip_camel_test_suffix(file_stem: &str) -> Option<&str> {
    file_stem
        .strip_suffix("Test")
        .filter(|s| s.ends_with(|c: char| c.is_ascii_alphanumeric()))
}

#[must_use]
pub(crate) fn is_screaming_snake(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() >= 2
        && bytes[0].is_ascii_uppercase()
        && bytes
            .iter()
            .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

#[must_use]
pub fn normalize_lexical_path(base_dir: &str, rel: &str) -> String {
    let combined = if base_dir.is_empty() {
        rel.to_string()
    } else {
        format!("{base_dir}/{rel}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for seg in combined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

pub(crate) fn probe_with_suffixes(
    candidate_base: &str,
    file_index: &HashMap<&str, u32>,
    suffixes: &[&str],
) -> Option<u32> {
    if let Some(&id) = file_index.get(candidate_base) {
        return Some(id);
    }
    let stripped = candidate_base
        .strip_suffix(".js")
        .or_else(|| candidate_base.strip_suffix(".jsx"))
        .or_else(|| candidate_base.strip_suffix(".mjs"))
        .unwrap_or(candidate_base);
    for sfx in suffixes {
        let key = format!("{stripped}{sfx}");
        if let Some(&id) = file_index.get(key.as_str()) {
            return Some(id);
        }
    }
    None
}

pub(crate) fn resolve_relative_or_exact(
    importer_path: &str,
    clean_target: &str,
    file_index: &HashMap<&str, u32>,
    suffixes: &[&str],
) -> Option<u32> {
    let base_dir = Path::new(importer_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    let rel_cand = normalize_lexical_path(base_dir, clean_target);
    if let Some(id) = probe_with_suffixes(&rel_cand, file_index, suffixes) {
        return Some(id);
    }
    if !clean_target.starts_with('.') {
        let root_cand = normalize_lexical_path("", clean_target);
        return probe_with_suffixes(&root_cand, file_index, suffixes);
    }
    None
}

/// The aliases a manifest at `rel_path` declares, when some language reads that manifest.
#[must_use]
pub fn import_aliases_in(rel_path: &str, content: &str) -> Vec<crate::domain::ImportAlias> {
    let name = rel_path.rsplit('/').next().unwrap_or(rel_path);
    let Some(lang) = ALL_LANGS
        .iter()
        .find(|l| l.semantics().import_alias_manifests().contains(&name))
    else {
        return Vec::new();
    };
    let dir = rel_path.rsplit_once('/').map_or("", |(d, _)| d);
    lang.semantics()
        .parse_import_aliases(content)
        .into_iter()
        .map(|(pattern, target)| crate::domain::ImportAlias {
            dir: dir.to_string(),
            pattern,
            target: normalize_lexical_path(dir, &target),
        })
        .collect()
}

/// Resolve `raw` imported from `importer_path`: the language's own rules, then the aliases of the
/// nearest manifest above the importer when the language reads such manifests.
#[must_use]
pub fn resolve_import(
    lang: Lang,
    importer_path: &str,
    raw: &str,
    file_index: &HashMap<&str, u32>,
    aliases: &[crate::domain::ImportAlias],
) -> Option<u32> {
    let semantics = lang.semantics();
    if let Some(id) = semantics.resolve_import_target(importer_path, raw, file_index) {
        return Some(id);
    }
    if semantics.import_alias_manifests().is_empty() {
        return None;
    }
    let applies = |a: &&crate::domain::ImportAlias| {
        a.dir.is_empty() || importer_path.starts_with(&format!("{}/", a.dir))
    };
    let nearest = aliases.iter().filter(applies).map(|a| a.dir.len()).max()?;
    let spec = strip_import_quotes(raw);
    aliases
        .iter()
        .filter(|a| a.dir.len() == nearest && applies(a))
        .filter_map(|a| match a.pattern.split_once('*') {
            Some((pre, post)) => spec
                .strip_prefix(pre)
                .and_then(|rest| rest.strip_suffix(post))
                .map(|mid| a.target.replacen('*', mid, 1)),
            None => (spec == a.pattern).then(|| a.target.clone()),
        })
        .find_map(|path| semantics.resolve_import_target("", &format!("./{path}"), file_index))
}
