use super::java::{JVM_STD_MEMBERS, JVM_STD_QUALIFIERS};
use super::{ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds};

pub struct ScalaLanguage;
pub static SCALA_LANG: ScalaLanguage = ScalaLanguage;

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &["case_clause", "type_case_clause"],
    flat: &[],
    nesting: &[
        "if_expression",
        "for_expression",
        "while_expression",
        "do_while_expression",
        "catch_clause",
    ],
    nest_only: &["match_expression"],
    else_kinds: &["else"],
    else_bodies: &[],
    jumps: &["return_expression", "throw_expression"],
    logical_nodes: &[],
    binary_nodes: &["infix_expression"],
    logical_ops: &["&&", "||"],
};

const SYNTAX: SyntaxKinds = SyntaxKinds {
    scope_containers: &["trait_definition", "class_definition", "object_definition"],
    member_access: &["field_expression"],
    field_access: &["field_expression"],
    body_blocks: &["block", "template_body", "indented_block"],
    body_wrappers: &["parenthesized_expression"],
    function_defs: &["function_declaration", "function_definition"],
    non_local_declarations: &["function_declaration"],
    identifiers: &["identifier"],
    type_contexts: &["type_parameters", "type_arguments", "generic_type"],
    callable_wrappers: &["parenthesized_expression"],
    binding_patterns: &["tuple_pattern"],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for ScalaLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn tag(&self) -> &'static str {
        "scala"
    }
    super::impl_grammar_method!(crate tree_sitter_scala::LANGUAGE, "../queries/scala/tags.scm");

    fn extensions(&self) -> &'static [&'static str] {
        &["scala", "sc"]
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Jvm
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        JVM_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        JVM_STD_QUALIFIERS
    }
}
