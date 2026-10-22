use super::javascript::{
    JS_KINDS, JS_STD_MEMBERS, JS_STD_QUALIFIERS, TS_CONFIG_MANIFESTS, is_const_scalar_binding,
    is_jsx_intrinsic_tag_call, is_std_member_call_value, parse_tsconfig_paths,
    resolve_web_import_target,
};
use super::{ComplexityKinds, DefCaptureGateContext, DialectGroup, LanguageSemantics, SyntaxKinds};
use crate::domain::Lang;
mod types;
pub mod astro;

use crate::ingest::{FileParseResult, parse_file_tree};
use std::collections::HashMap;
use std::sync::LazyLock;
use tree_sitter::{Language, Query};

pub struct TypeScriptLanguage;
pub static TYPESCRIPT_LANG: TypeScriptLanguage = TypeScriptLanguage;

static TSX_LANG_AND_QUERY: LazyLock<(Language, Query)> = LazyLock::new(|| {
    let ts: Language = tree_sitter_typescript::LANGUAGE_TSX.into();
    let src = concat!(
        include_str!("tags.scm"),
        "\n(jsx_self_closing_element name: (identifier) @name) @reference.call\n",
        "(jsx_self_closing_element name: (member_expression property: (property_identifier) @name)) @reference.call\n",
        "(jsx_opening_element name: (identifier) @name) @reference.call\n",
        "(jsx_opening_element name: (member_expression property: (property_identifier) @name)) @reference.call\n"
    );
    let q = Query::new(&ts, src).expect("valid tsx query");
    (ts, q)
});

fn parse_astro_file(source_bytes: &[u8], language: &Language, query: &Query) -> FileParseResult {
    let astro::AstroFrontmatter::Ok {
        start_byte,
        end_byte,
        start_row,
    } = astro::extract_astro_frontmatter(source_bytes)
    else {
        return FileParseResult::default();
    };
    let sub_bytes = &source_bytes[start_byte..end_byte];
    let mut res = parse_file_tree(sub_bytes, language, query, Lang::TypeScript);
    for d in &mut res.defs {
        d.line += start_row;
        d.start_byte += start_byte;
        d.end_byte += start_byte;
    }
    for r in &mut res.refs {
        r.line += start_row;
        r.start_byte += start_byte;
    }
    res.line_count = source_bytes.iter().filter(|&&b| b == b'\n').count() as u32 + 1;
    res.byte_size = source_bytes.len() as u64;
    res
}

const SYNTAX: SyntaxKinds = SyntaxKinds {
    scope_containers: &[
        "class",
        "class_declaration",
        "abstract_class_declaration",
        "enum_declaration",
        "interface_declaration",
        "module",
    ],
    member_access: &["member_expression"],
    field_access: &["member_expression"],
    body_blocks: &["statement_block", "class_body"],
    body_wrappers: &[
        "lexical_declaration",
        "variable_declaration",
        "variable_declarator",
        "public_field_definition",
        "export_statement",
        "arrow_function",
        "function_expression",
        "generator_function",
        "as_expression",
        "satisfies_expression",
        "parenthesized_expression",
    ],
    function_defs: &["function_declaration", "method_definition"],
    local_bindings: &["for_in_statement"],
    non_local_declarations: &[
        "function_declaration",
        "class_declaration",
        "interface_declaration",
        "type_alias_declaration",
        "enum_declaration",
    ],
    identifiers: &["identifier", "shorthand_property_identifier_pattern"],
    type_contexts: &[
        "type_parameters",
        "type_parameter",
        "type_annotation",
        "type_arguments",
        "generic_type",
    ],
    callables: &["arrow_function", "function_expression"],
    callable_wrappers: &[
        "variable_declarator",
        "as_expression",
        "satisfies_expression",
        "parenthesized_expression",
    ],
    signature_holders: &["method_signature"],
    binding_patterns: &[
        "object_pattern",
        "array_pattern",
        "pair_pattern",
        "rest_pattern",
        "assignment_pattern",
    ],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for TypeScriptLanguage {
    fn env_reads(&self) -> &'static [crate::languages::common::flags::EnvRead] {
        &[crate::languages::common::flags::EnvRead::Member("process.env")]
    }

    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &JS_KINDS
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn has_declarations(&self) -> bool {
        true
    }

    /// Overload signatures precede their implementation in the same file and scope.
    fn declaration_ties(&self, decl: &crate::domain::Symbol, def: &crate::domain::Symbol) -> bool {
        decl.scope == def.scope && decl.file_id == def.file_id
    }

    fn tag(&self) -> &'static str {
        "ts"
    }

    fn lint_groups(&self) -> &'static [crate::languages::LintGroup] {
        use crate::languages::LintGroup as G;
        &[G::DoWhile, G::Insecure, G::Parens, G::SelfAssign, G::Unreachable, G::Naming]
    }
    super::impl_grammar_method!(crate tree_sitter_typescript::LANGUAGE_TYPESCRIPT, "tags.scm");

    fn is_read_capture_allowed(&self, node: tree_sitter::Node<'_>) -> bool {
        super::javascript::javascript_read_position(node)
    }

    fn read_bindings<'a>(
        &self,
        root: tree_sitter::Node<'_>,
        source: &'a [u8],
    ) -> super::reads::ReadBindings<'a> {
        types::typed_read_bindings(root, source)
    }

    fn implementations(
        &self,
        root: tree_sitter::Node<'_>,
        source: &[u8],
    ) -> Vec<super::Implementation> {
        types::class_implementations(root, source)
    }

    fn indexes_implementations(&self) -> bool {
        true
    }

    fn unindexed_receiver_type_uses_names(&self, receiver_type: &str) -> bool {
        // Object-literal and plain-name types the index does not know: name resolution, as before.
        receiver_type.starts_with('{')
            || receiver_type
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }

    fn undeclared_call_receiver_declines(&self) -> bool {
        true
    }

    fn interface_calls_keep_declaration(&self) -> bool {
        true
    }

    fn member_receiver_type(&self, receiver_type: &str, member: &str) -> Option<String> {
        types::object_member_type(receiver_type, member)
    }

    fn lint_return_type<'a>(&self, sig: &'a str, _name: &str) -> Option<&'a str> {
        super::lint_hooks::colon_return_type(sig)
    }

    fn lint_bool_types(&self) -> &'static [&'static str] {
        &["boolean"]
    }

    fn type_declaration_keywords(&self) -> &'static [&'static str] {
        &["type", "interface"]
    }

    fn source_grammar(&self, path: &str) -> Option<&'static Language> {
        if path.ends_with(".tsx") {
            Some(&TSX_LANG_AND_QUERY.0)
        } else {
            self.grammar_and_query().map(|(language, _)| language)
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx", "mts", "cts", "astro"]
    }

    fn test_runner_manifest(&self) -> Option<&'static str> {
        Some("package.json")
    }

    fn test_run_command(&self, path: &str, source: &str, manifest: Option<&str>) -> Option<String> {
        super::javascript::js_test_run_command(path, source, manifest, false)
    }

    fn display_color(&self) -> &'static str {
        "#3178c6"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Web
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        match ctx.sub {
            Some("constant") => super::is_screaming_snake(ctx.name),
            None => ctx.kind != crate::domain::SymKind::Function || !is_std_member_call_value(ctx),
            _ => true,
        }
    }

    fn is_immutable_scalar_binding(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        is_const_scalar_binding(ctx)
    }

    fn should_ignore_call(&self, name_node: Option<tree_sitter::Node<'_>>, name: &str) -> bool {
        is_jsx_intrinsic_tag_call(name_node, name)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        JS_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        JS_STD_QUALIFIERS
    }

    fn parse_file_override(
        &self,
        source_bytes: &[u8],
        rel_path: &str,
        language: &Language,
        query: &Query,
    ) -> Option<FileParseResult> {
        if rel_path.ends_with(".tsx") {
            let (tsx_lang, tsx_query) = &*TSX_LANG_AND_QUERY;
            return Some(parse_file_tree(
                source_bytes,
                tsx_lang,
                tsx_query,
                Lang::TypeScript,
            ));
        }
        if !rel_path.ends_with(".astro") {
            return None;
        }
        Some(parse_astro_file(source_bytes, language, query))
    }

    fn import_alias_manifests(&self) -> &'static [&'static str] {
        TS_CONFIG_MANIFESTS
    }

    fn parse_import_aliases(&self, content: &str) -> Vec<(String, String)> {
        parse_tsconfig_paths(content)
    }

    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        resolve_web_import_target(importer_path, raw_target, file_index)
    }
}
