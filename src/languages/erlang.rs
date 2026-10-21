use super::elixir::{BEAM_STD_MEMBERS, BEAM_STD_QUALIFIERS};
use super::{ComplexityKinds, DialectGroup, LanguageSemantics};

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

impl LanguageSemantics for ErlangLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }

    fn tag(&self) -> &'static str {
        "erl"
    }
    super::impl_grammar_method!(crate tree_sitter_erlang::LANGUAGE, "../queries/erlang/tags.scm");

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
