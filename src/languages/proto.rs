use super::{DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct ProtoLanguage;
pub static PROTO_LANG: ProtoLanguage = ProtoLanguage;

impl LanguageSemantics for ProtoLanguage {
    fn tag(&self) -> &'static str {
        "proto"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Proto)
    }

    fn supports_slice(&self) -> bool {
        false
    }
}
