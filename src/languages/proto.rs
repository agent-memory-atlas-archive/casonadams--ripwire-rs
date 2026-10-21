use super::{ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds};
use crate::domain::Lang;

pub struct ProtoLanguage;
pub static PROTO_LANG: ProtoLanguage = ProtoLanguage;

const SYNTAX: SyntaxKinds = SyntaxKinds {
    identifiers: &["identifier"],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for ProtoLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &ComplexityKinds::NONE
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn tag(&self) -> &'static str {
        "proto"
    }
    super::impl_grammar_method!(crate tree_sitter_proto::LANGUAGE, "../queries/proto/tags.scm");

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Proto)
    }

    fn supports_slice(&self) -> bool {
        false
    }

    fn retrieval_weight(&self) -> f64 {
        0.35
    }
}
