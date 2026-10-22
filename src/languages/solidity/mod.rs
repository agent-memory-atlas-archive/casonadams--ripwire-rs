use super::{ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds};
use crate::domain::Lang;

pub struct SolidityLanguage;
pub static SOLIDITY_LANG: SolidityLanguage = SolidityLanguage;

const SOLIDITY_STD_MEMBERS: &[&str] = &[
    "push",
    "pop",
    "transfer",
    "send",
    "call",
    "delegatecall",
    "staticcall",
    "encode",
    "encodePacked",
    "encodeWithSelector",
    "encodeWithSignature",
    "encodeCall",
    "decode",
    "concat",
];

const SOLIDITY_STD_QUALIFIERS: &[&str] =
    &["abi", "msg", "block", "tx", "address", "string", "bytes"];

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &[],
    flat: &[],
    nesting: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "do_while_statement",
        "catch_clause",
        "ternary_expression",
        "yul_if_statement",
        "yul_for_statement",
        "yul_switch_statement",
    ],
    nest_only: &[],
    else_kinds: &["else"],
    else_bodies: &[],
    jumps: &[
        "break_statement",
        "continue_statement",
        "return_statement",
        "revert_statement",
        "yul_break",
        "yul_continue",
        "yul_leave",
    ],
    logical_nodes: &[],
    binary_nodes: &["binary_expression"],
    logical_ops: &["&&", "||"],
};

const SYNTAX: SyntaxKinds = SyntaxKinds {
    scope_containers: &[
        "struct_declaration",
        "enum_declaration",
        "interface_declaration",
        "contract_declaration",
        "library_declaration",
    ],
    member_access: &["member_expression"],
    field_access: &["member_expression"],
    body_blocks: &["function_body"],
    body_wrappers: &["variable_declaration", "parenthesized_expression"],
    function_defs: &["function_definition"],
    non_local_declarations: &["interface_declaration", "enum_declaration"],
    identifiers: &["identifier"],
    callable_wrappers: &["parenthesized_expression"],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for SolidityLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn tag(&self) -> &'static str {
        "sol"
    }
    super::impl_grammar_method!(crate tree_sitter_solidity::LANGUAGE, "tags.scm");

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Solidity)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        SOLIDITY_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        SOLIDITY_STD_QUALIFIERS
    }
}
