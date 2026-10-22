use super::elixir::{BEAM_STD_MEMBERS, BEAM_STD_QUALIFIERS};
use super::{ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds};

pub struct ErlangLanguage;
pub static ERLANG_LANG: ErlangLanguage = ErlangLanguage;

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &["cr_clause", "if_clause"],
    flat: &["catch_clause"],
    nesting: &["try_expr", "catch_expr"],
    nest_only: &["case_expr", "if_expr", "receive_expr"],
    else_kinds: &[],
    else_bodies: &[],
    jumps: &[],
    logical_nodes: &[],
    binary_nodes: &["binary_op_expr"],
    logical_ops: &["andalso", "orelse"],
};

const SYNTAX: SyntaxKinds = SyntaxKinds {
    scope_containers: &["module"],
    member_access: &["remote"],
    body_blocks: &["clause_body"],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for ErlangLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn member_receiver<'a>(&self, access: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
        match access.kind() {
            "remote" => access
                .child_by_field_name("module")
                .and_then(|m| m.child_by_field_name("module")),
            _ => None,
        }
    }

    fn tag(&self) -> &'static str {
        "erl"
    }
    super::impl_grammar_method!(crate tree_sitter_erlang::LANGUAGE, "tags.scm");

    fn extensions(&self) -> &'static [&'static str] {
        &["erl", "hrl"]
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Beam
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        BEAM_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        BEAM_STD_QUALIFIERS
    }
}
