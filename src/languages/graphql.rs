use super::{DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct GraphqlLanguage;
pub static GRAPHQL_LANG: GraphqlLanguage = GraphqlLanguage;

impl LanguageSemantics for GraphqlLanguage {
    fn tag(&self) -> &'static str {
        "gql"
    }
    super::impl_grammar_method!(ffi tree_sitter_graphql, include_str!("../queries/graphql/tags.scm"));

    fn extensions(&self) -> &'static [&'static str] {
        &["graphql", "gql"]
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Graphql)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }
}
