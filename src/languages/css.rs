use super::{DialectGroup, LanguageSemantics};

pub struct CssLanguage;
pub static CSS_LANG: CssLanguage = CssLanguage;

impl LanguageSemantics for CssLanguage {
    fn tag(&self) -> &'static str {
        "css"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Markup
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }
}
