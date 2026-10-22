pub mod embedded;

use super::{ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds};
use crate::domain::Lang;

pub struct SqlLanguage;
pub static SQL_LANG: SqlLanguage = SqlLanguage;

const SYNTAX: SyntaxKinds = SyntaxKinds {
    body_blocks: &["function_body", "block"],
    body_wrappers: &["parenthesized_expression"],
    function_defs: &["function_declaration"],
    non_local_declarations: &["function_declaration"],
    identifiers: &["identifier"],
    callable_wrappers: &["parenthesized_expression"],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for SqlLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &ComplexityKinds::NONE
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn tag(&self) -> &'static str {
        "sql"
    }
    super::impl_grammar_method!(ffi tree_sitter_sql, include_str!("tags.scm"));

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Sql)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }

    fn embedded_refs(
        &self,
        root: tree_sitter::Node<'_>,
        source: &[u8],
        refs: &mut Vec<crate::ingest::RawRef>,
    ) {
        if embedded::has_dml_anchor(source) {
            embedded::extract_refs(root, source, refs);
        }
    }

    fn supports_embedded_sql(&self) -> bool {
        false
    }

    fn is_table_read_write_target(&self) -> bool {
        true
    }
}
