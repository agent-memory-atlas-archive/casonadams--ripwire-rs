use super::{ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds};
use crate::domain::Lang;

pub struct JsonLanguage;
pub static JSON_LANG: JsonLanguage = JsonLanguage;

impl LanguageSemantics for JsonLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &ComplexityKinds::NONE
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SyntaxKinds::NONE
    }

    fn tag(&self) -> &'static str {
        "json"
    }
    super::impl_grammar_method!(crate tree_sitter_json::LANGUAGE, "tags.scm");

    fn display_color(&self) -> &'static str {
        "#292929"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Json)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn is_data_format(&self) -> bool {
        true
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
