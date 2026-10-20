use super::{DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::Lang;

pub struct BashLanguage;
pub static BASH_LANG: BashLanguage = BashLanguage;

impl LanguageSemantics for BashLanguage {
    fn tag(&self) -> &'static str {
        "sh"
    }
    super::impl_grammar_method!(crate tree_sitter_bash::LANGUAGE, "../queries/bash/tags.scm");

    fn extensions(&self) -> &'static [&'static str] {
        &["sh", "bash", "zsh"]
    }

    fn display_color(&self) -> &'static str {
        "#89e051"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Bash)
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn quoted_heredoc_terminator<'a>(&self, line: &'a str) -> Option<&'a str> {
        let rest = line.split_once("<<")?.1;
        let rest = rest.strip_prefix('-').unwrap_or(rest).trim_start();
        let quote = rest.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
        let word = rest[1..].split(quote).next()?;
        (!word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
            .then_some(word)
    }

    fn supports_slice(&self) -> bool {
        false
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }
}
