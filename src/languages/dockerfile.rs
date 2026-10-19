use super::{DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct DockerfileLanguage;
pub static DOCKERFILE_LANG: DockerfileLanguage = DockerfileLanguage;

impl LanguageSemantics for DockerfileLanguage {
    fn tag(&self) -> &'static str {
        "docker"
    }
    super::impl_grammar_method!(ffi tree_sitter_dockerfile, include_str!("../queries/dockerfile/tags.scm"));

    fn extensions(&self) -> &'static [&'static str] {
        &["dockerfile", "containerfile"]
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Dockerfile)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }
}
