use super::{DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::Lang;

pub struct GDScriptLanguage;
pub static GDSCRIPT_LANG: GDScriptLanguage = GDScriptLanguage;

const GDSCRIPT_STD_MEMBERS: &[&str] = &[
    "append",
    "erase",
    "find",
    "has",
    "is_empty",
    "size",
    "clear",
    "duplicate",
    "keys",
    "values",
    "get",
    "set",
    "get_node",
    "add_child",
    "remove_child",
    "queue_free",
    "instantiate",
    "connect",
    "emit_signal",
];

impl LanguageSemantics for GDScriptLanguage {
    fn tag(&self) -> &'static str {
        "gd"
    }
    super::impl_grammar_method!(ffi tree_sitter_gdscript, include_str!("../queries/gdscript/tags.scm"));

    fn display_color(&self) -> &'static str {
        "#355570"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::GDScript)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        GDSCRIPT_STD_MEMBERS
    }
}
