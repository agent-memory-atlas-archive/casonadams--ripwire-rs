use super::{ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds, VisibilityMode};
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

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &["pattern_section"],
    flat: &["elif_clause"],
    nesting: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "conditional_expression",
    ],
    nest_only: &["match_statement"],
    else_kinds: &["else_clause"],
    else_bodies: &[],
    jumps: &["break_statement", "continue_statement", "return_statement"],
    logical_nodes: &[],
    binary_nodes: &["binary_operator"],
    logical_ops: &["&&", "||", "and", "or"],
};

const SYNTAX: SyntaxKinds = SyntaxKinds {
    scope_containers: &["class_definition"],
    member_access: &["attribute"],
    field_access: &["attribute"],
    body_blocks: &["class_body"],
    body_wrappers: &["parenthesized_expression"],
    function_defs: &["function_definition"],
    identifiers: &["identifier", "name"],
    callable_wrappers: &["parenthesized_expression"],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for GDScriptLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn tag(&self) -> &'static str {
        "gd"
    }
    super::impl_grammar_method!(ffi tree_sitter_gdscript, include_str!("tags.scm"));

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
