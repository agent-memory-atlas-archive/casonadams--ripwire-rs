use super::{DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::Lang;

pub struct LuaLanguage;
pub static LUA_LANG: LuaLanguage = LuaLanguage;

const LUA_STD_MEMBERS: &[&str] = &[
    "insert", "remove", "concat", "sort", "unpack", "pack", "sub", "find", "match", "gmatch",
    "gsub", "rep", "reverse", "lower", "upper", "len", "byte", "char", "format", "floor", "ceil",
    "max", "min", "abs", "sqrt", "open", "close", "read", "write", "lines", "flush",
];

const LUA_STD_QUALIFIERS: &[&str] = &[
    "table",
    "string",
    "math",
    "io",
    "os",
    "coroutine",
    "package",
    "debug",
    "utf8",
    "bit",
    "vim",
];

impl LanguageSemantics for LuaLanguage {
    fn tag(&self) -> &'static str {
        "lua"
    }
    super::impl_grammar_method!(ffi tree_sitter_lua, include_str!("../queries/lua/tags.scm"));

    fn display_color(&self) -> &'static str {
        "#000080"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Lua)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }

    fn extract_custom_scope(
        &self,
        parent: tree_sitter::Node<'_>,
        name_node: tree_sitter::Node<'_>,
        source_bytes: &[u8],
    ) -> Option<String> {
        if !matches!(
            parent.kind(),
            "dot_index_expression" | "method_index_expression"
        ) {
            return None;
        }
        let gp_kind = parent.parent()?.kind();
        if !matches!(gp_kind, "function_declaration" | "variable_list") {
            return None;
        }
        crate::ingest::distinct_scope_tail(
            parent.child_by_field_name("table")?,
            name_node,
            source_bytes,
            "",
        )
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        LUA_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        LUA_STD_QUALIFIERS
    }
}
