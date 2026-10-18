use super::{DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct JsonLanguage;
pub static JSON_LANG: JsonLanguage = JsonLanguage;

impl LanguageSemantics for JsonLanguage {
    fn tag(&self) -> &'static str {
        "json"
    }

    fn display_color(&self) -> &'static str {
        "#292929"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Json)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }

    fn supports_embedded_sql(&self) -> bool {
        false
    }

    fn max_file_size(&self, default_max: u64) -> u64 {
        default_max.min(crate::crawler::DEFAULT_JSON_CEILING)
    }
}
