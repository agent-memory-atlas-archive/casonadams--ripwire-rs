use super::{ComplexityKinds, DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct TomlLanguage;
pub static TOML_LANG: TomlLanguage = TomlLanguage;

impl LanguageSemantics for TomlLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &ComplexityKinds::NONE
    }

    fn tag(&self) -> &'static str {
        "toml"
    }
    super::impl_grammar_method!(ffi tree_sitter_toml, include_str!("../queries/toml/tags.scm"));

    fn display_color(&self) -> &'static str {
        "#9c4221"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Toml)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn is_data_format(&self) -> bool {
        true
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn supports_slice(&self) -> bool {
        false
    }

    fn max_file_size(&self, default_max: u64) -> u64 {
        default_max.min(crate::crawler::DEFAULT_YAML_CEILING)
    }
}
