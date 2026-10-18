pub mod c_family;
pub mod data_and_markup;
pub mod functional_and_systems;
pub mod jvm;
pub mod rust;
pub mod scripting;
pub mod web;

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
            .map(crate::gate::contract::extract_param_count)
            .unwrap_or(0);
        (params, orig_name.to_string())
    }

    /// True if a captured identifier is a language keyword or cast operator that must not be indexed.
    fn is_builtin_keyword(&self, _name: &str) -> bool {
        false
    }

    /// True if a captured call reference is a language builtin/macro that should not form a user call edge.
    fn should_ignore_call(&self, _name_node: Option<tree_sitter::Node<'_>>, name: &str) -> bool {
        self.is_builtin_keyword(name)
    }

    /// True if a method name belongs to the language's standard container/string/builtin types (declined on untyped receivers).
    fn is_std_or_builtin_member(&self, _name: &str) -> bool {
        false
    }

    /// True if an explicit call qualifier names a standard library or builtin runtime namespace/type.
    fn is_std_or_builtin_qualifier(&self, _qualifier: &str) -> bool {
        false
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

    /// True if an AST definition node has public visibility in this language.
    fn check_node_visibility(&self, def_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
        match self.visibility_mode() {
            VisibilityMode::ExplicitModifier => has_any_visibility_modifier(def_node, source_bytes),
            VisibilityMode::ByNameConvention => {
                check_leading_underscore_private(def_node, source_bytes)
            }
            VisibilityMode::DefaultPublic => true,
        }
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
}

static SEMANTICS_TABLE: [&dyn LanguageSemantics; 37] = [
    &rust::RUST_LANG,
    &web::TYPESCRIPT_LANG,
    &web::JAVASCRIPT_LANG,
    &scripting::PYTHON_LANG,
    &functional_and_systems::GO_LANG,
    &c_family::C_LANG,
    &c_family::CPP_LANG,
    &c_family::CUDA_LANG,
    &c_family::METAL_LANG,
    &jvm::JAVA_LANG,
    &jvm::CSHARP_LANG,
    &jvm::SWIFT_LANG,
    &scripting::BASH_LANG,
    &scripting::RUBY_LANG,
    &jvm::PHP_LANG,
    &scripting::LUA_LANG,
    &functional_and_systems::ELIXIR_LANG,
    &functional_and_systems::DART_LANG,
    &jvm::KOTLIN_LANG,
    &scripting::GDSCRIPT_LANG,
    &data_and_markup::JSON_LANG,
    &data_and_markup::TOML_LANG,
    &data_and_markup::YAML_LANG,
    &data_and_markup::MARKDOWN_LANG,
    &functional_and_systems::ZIG_LANG,
    &functional_and_systems::SOLIDITY_LANG,
    &data_and_markup::HCL_LANG,
    &jvm::SCALA_LANG,
    &functional_and_systems::ERLANG_LANG,
    &web::HTML_LANG,
    &web::CSS_LANG,
    &data_and_markup::SQL_LANG,
    &data_and_markup::PROTO_LANG,
    &data_and_markup::GRAPHQL_LANG,
    &data_and_markup::DOCKERFILE_LANG,
    &c_family::OBJC_LANG,
    &data_and_markup::UNKNOWN_LANG,
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
) -> bool {
    let node = crate::gate::contract::unwrap_signature_node(def_node);
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
