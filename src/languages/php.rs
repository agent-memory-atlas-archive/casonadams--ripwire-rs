use super::{ComplexityKinds, DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::Lang;

pub struct PhpLanguage;
pub static PHP_LANG: PhpLanguage = PhpLanguage;

const PHP_STD_MEMBERS: &[&str] = &[
    "count",
    "sizeof",
    "in_array",
    "array_map",
    "array_filter",
    "array_merge",
    "array_keys",
    "array_values",
    "array_push",
    "array_pop",
    "array_shift",
    "array_slice",
    "implode",
    "explode",
    "trim",
    "strlen",
    "strpos",
    "substr",
    "str_replace",
    "strtolower",
    "strtoupper",
    "sprintf",
    "json_encode",
    "json_decode",
];

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &["case_statement", "match_conditional_expression"],
    flat: &["else_if_clause"],
    nesting: &[
        "if_statement",
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_statement",
        "catch_clause",
        "conditional_expression",
    ],
    nest_only: &["switch_statement", "match_expression"],
    else_kinds: &["else_clause"],
    else_bodies: &[],
    jumps: &[
        "break_statement",
        "continue_statement",
        "goto_statement",
        "return_statement",
        "throw_expression",
    ],
    logical_nodes: &[],
    binary_nodes: &["binary_expression"],
    logical_ops: &["&&", "||", "and", "or"],
};

impl LanguageSemantics for PhpLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }

    fn tag(&self) -> &'static str {
        "php"
    }
    super::impl_grammar_method!(ffi tree_sitter_php, include_str!("../queries/php/tags.scm"));

    fn lint_extra_rules(&self) -> &'static [&'static str] {
        &[
            "c-style-cast",
            "redundant-parens",
            "do-while",
            "self-assign",
            "unreachable-code",
        ]
    }

    fn type_declaration_keywords(&self) -> &'static [&'static str] {
        &["enum"]
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["php", "phtml"]
    }

    fn camel_test_partner_subject<'a>(&self, file_stem: &'a str) -> Option<&'a str> {
        super::strip_camel_test_suffix(file_stem)
    }

    fn display_color(&self) -> &'static str {
        "#4f5d95"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Php)
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        PHP_STD_MEMBERS
    }
}
