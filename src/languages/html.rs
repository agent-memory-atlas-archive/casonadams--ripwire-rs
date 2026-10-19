use super::{DialectGroup, LanguageSemantics};

pub struct HtmlLanguage;
pub static HTML_LANG: HtmlLanguage = HtmlLanguage;

impl LanguageSemantics for HtmlLanguage {
    fn tag(&self) -> &'static str {
        "html"
    }
    super::impl_grammar_method!(crate tree_sitter_html::LANGUAGE, "../queries/html/tags.scm");

    fn extensions(&self) -> &'static [&'static str] {
        &["html", "htm"]
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
