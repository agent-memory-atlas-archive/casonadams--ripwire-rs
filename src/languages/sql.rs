use super::{ComplexityKinds, DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct SqlLanguage;
pub static SQL_LANG: SqlLanguage = SqlLanguage;

impl LanguageSemantics for SqlLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &ComplexityKinds::NONE
    }

    fn tag(&self) -> &'static str {
        "sql"
    }
    super::impl_grammar_method!(ffi tree_sitter_sql, include_str!("../queries/sql/tags.scm"));

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Sql)
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

    fn is_table_read_write_target(&self) -> bool {
        true
    }
}
