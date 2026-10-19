use super::c::{
    C_FAMILY_KEYWORDS, CPP_STD_MEMBERS, CPP_STD_QUALIFIERS, blank_c_family_macro_lines,
    is_c_family_def_allowed, refine_c_header_dialect, resolve_c_family_def_body_and_end,
    resolve_c_family_include,
};
use super::{DefCaptureGateContext, DialectGroup, LanguageSemantics};
use std::collections::HashMap;

pub struct CppLanguage;
pub static CPP_LANG: CppLanguage = CppLanguage;

impl LanguageSemantics for CppLanguage {
    fn tag(&self) -> &'static str {
        "cpp"
    }
    super::impl_grammar_method!(crate tree_sitter_cpp::LANGUAGE, "../queries/cpp/tags.scm");

    fn extensions(&self) -> &'static [&'static str] {
        &["h", "cpp", "cc", "cxx", "hpp", "hxx", "hh"]
    }

    fn display_color(&self) -> &'static str {
        "#f34b7d"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
    }

    fn supports_c_layout(&self) -> bool {
        true
    }

    fn refine_dialect(
        &self,
        base_lang: crate::domain::Lang,
        rel_path: &str,
        source_bytes: &[u8],
    ) -> crate::domain::Lang {
        refine_c_header_dialect(base_lang, rel_path, source_bytes)
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

    fn blank_error_macros(&self, source_bytes: &[u8]) -> Option<Vec<u8>> {
        blank_c_family_macro_lines(source_bytes)
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
