use super::{DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct ProtoLanguage;
pub static PROTO_LANG: ProtoLanguage = ProtoLanguage;

impl LanguageSemantics for ProtoLanguage {
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
