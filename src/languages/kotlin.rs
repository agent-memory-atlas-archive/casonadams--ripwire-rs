use super::java::{JVM_STD_MEMBERS, JVM_STD_QUALIFIERS};
use super::{DialectGroup, LanguageSemantics, VisibilityMode};

pub struct KotlinLanguage;
pub static KOTLIN_LANG: KotlinLanguage = KotlinLanguage;

impl LanguageSemantics for KotlinLanguage {
    fn tag(&self) -> &'static str {
        "kt"
    }
    super::impl_grammar_method!(ffi tree_sitter_kotlin, include_str!("../queries/kotlin/tags.scm"));

    fn lint_extra_rules(&self) -> &'static [&'static str] {
        &["redundant-parens"]
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["kt", "kts"]
    }

    fn camel_test_partner_subject<'a>(&self, file_stem: &'a str) -> Option<&'a str> {
        super::strip_camel_test_suffix(file_stem)
    }

    fn display_color(&self) -> &'static str {
        "#a97bff"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Jvm
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        JVM_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        JVM_STD_QUALIFIERS
    }
}
