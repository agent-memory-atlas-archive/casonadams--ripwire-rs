use super::elixir::{BEAM_STD_MEMBERS, BEAM_STD_QUALIFIERS};
use super::{DialectGroup, LanguageSemantics};

pub struct ErlangLanguage;
pub static ERLANG_LANG: ErlangLanguage = ErlangLanguage;

impl LanguageSemantics for ErlangLanguage {
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
