use super::c::{
    C_FAMILY_KEYWORDS, CPP_STD_MEMBERS, CPP_STD_QUALIFIERS, is_c_family_def_allowed,
    resolve_c_family_def_body_and_end, resolve_c_family_include,
};
use super::{ComplexityKinds, DefCaptureGateContext, DialectGroup, LanguageSemantics};
use std::collections::HashMap;

pub struct MetalLanguage;
pub static METAL_LANG: MetalLanguage = MetalLanguage;

const KINDS: ComplexityKinds = ComplexityKinds {
    nesting: &[
        "if_statement",
        "for_statement",
        "for_range_loop",
        "while_statement",
        "do_statement",
        "catch_clause",
        "conditional_expression",
    ],
    jumps: &[
        "break_statement",
        "continue_statement",
        "goto_statement",
        "return_statement",
        "throw_statement",
        "co_return_statement",
    ],
    ..ComplexityKinds::C_STYLE
};

impl LanguageSemantics for MetalLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }
    fn tag(&self) -> &'static str {
        "metal"
    }
    super::impl_grammar_method!(crate tree_sitter_cpp::LANGUAGE, "../queries/cpp/tags.scm");

    fn lint_tag(&self) -> &'static str {
        "cpp"
    }

    fn lint_return_type<'a>(&self, sig: &'a str, name: &str) -> Option<&'a str> {
        super::lint_hooks::prefix_return_type(sig, name)
    }

    fn type_declaration_keywords(&self) -> &'static [&'static str] {
        &["typedef", "enum", "using"]
    }

    fn supports_cache_lint(&self) -> bool {
        true
    }

    fn display_color(&self) -> &'static str {
        "#3a87ad"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
    }

    fn is_builtin_keyword(&self, name: &str) -> bool {
        C_FAMILY_KEYWORDS.contains(&name)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        CPP_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        CPP_STD_QUALIFIERS
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        is_c_family_def_allowed(ctx, true)
    }

    fn resolve_def_body_and_end<'a>(
        &self,
        def_node: Option<tree_sitter::Node<'a>>,
        default_end: usize,
    ) -> (Option<tree_sitter::Node<'a>>, usize) {
        resolve_c_family_def_body_and_end(def_node, default_end)
    }

    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        resolve_c_family_include(importer_path, raw_target, file_index)
    }
}
