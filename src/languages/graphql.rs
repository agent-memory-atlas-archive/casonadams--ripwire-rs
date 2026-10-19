use super::{DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct GraphqlLanguage;
pub static GRAPHQL_LANG: GraphqlLanguage = GraphqlLanguage;

impl LanguageSemantics for GraphqlLanguage {
    fn tag(&self) -> &'static str {
        "gql"
    }

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
