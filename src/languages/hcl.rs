use super::{DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct HclLanguage;
pub static HCL_LANG: HclLanguage = HclLanguage;

impl LanguageSemantics for HclLanguage {
    fn tag(&self) -> &'static str {
        "hcl"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Hcl)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }
}
