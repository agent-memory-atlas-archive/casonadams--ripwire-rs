use super::{ComplexityKinds, DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct GraphqlLanguage;
pub static GRAPHQL_LANG: GraphqlLanguage = GraphqlLanguage;

impl LanguageSemantics for GraphqlLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &ComplexityKinds::NONE
    }

    fn tag(&self) -> &'static str {
        "gql"
    }
    super::impl_grammar_method!(crate tree_sitter_graphql::LANGUAGE, "../queries/graphql/tags.scm");

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
