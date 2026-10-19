use super::{DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::Lang;

pub struct PhpLanguage;
pub static PHP_LANG: PhpLanguage = PhpLanguage;

const PHP_STD_MEMBERS: &[&str] = &[
    "count",
    "sizeof",
    "in_array",
    "array_map",
    "array_filter",
    "array_merge",
    "array_keys",
    "array_values",
    "array_push",
    "array_pop",
    "array_shift",
    "array_slice",
    "implode",
    "explode",
    "trim",
    "strlen",
    "strpos",
    "substr",
    "str_replace",
    "strtolower",
    "strtoupper",
    "sprintf",
    "json_encode",
    "json_decode",
];

impl LanguageSemantics for PhpLanguage {
    fn tag(&self) -> &'static str {
        "php"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["php", "phtml"]
    }

    fn display_color(&self) -> &'static str {
        "#4f5d95"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Php)
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        PHP_STD_MEMBERS
    }
}
