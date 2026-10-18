use super::c::{
    C_FAMILY_KEYWORDS, CPP_STD_MEMBERS, CPP_STD_QUALIFIERS, is_c_family_def_allowed,
    resolve_c_family_def_body_and_end, resolve_c_family_include,
};
use super::{DefCaptureGateContext, DialectGroup, LanguageSemantics};
use std::collections::HashMap;

pub struct MetalLanguage;
pub static METAL_LANG: MetalLanguage = MetalLanguage;

impl LanguageSemantics for MetalLanguage {
    fn tag(&self) -> &'static str {
        "metal"
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
