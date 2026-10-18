use super::{DialectGroup, LanguageSemantics};
use crate::domain::Lang;
use crate::ingest::{FileParseResult, markdown};
use tree_sitter::{Language, Query};

pub struct MarkdownLanguage;
pub static MARKDOWN_LANG: MarkdownLanguage = MarkdownLanguage;

impl LanguageSemantics for MarkdownLanguage {
    fn tag(&self) -> &'static str {
        "md"
    }

    fn display_color(&self) -> &'static str {
        "#083fa1"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Markdown)
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

    fn is_prose_document(&self) -> bool {
        true
    }

    fn parse_file_override(
        &self,
        source_bytes: &[u8],
        rel_path: &str,
        language: &Language,
        _query: &Query,
    ) -> Option<FileParseResult> {
        Some(markdown::extract_markdown(source_bytes, rel_path, language))
    }
}
