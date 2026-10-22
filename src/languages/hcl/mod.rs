use super::{ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds};
use crate::domain::Lang;

pub struct HclLanguage;
pub static HCL_LANG: HclLanguage = HclLanguage;

const SYNTAX: SyntaxKinds = SyntaxKinds {
    body_blocks: &["block"],
    identifiers: &["identifier"],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for HclLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &ComplexityKinds::NONE
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn tag(&self) -> &'static str {
        "hcl"
    }
    super::impl_grammar_method!(crate tree_sitter_hcl::LANGUAGE, "tags.scm");

    fn extensions(&self) -> &'static [&'static str] {
        &["tf", "hcl", "tfvars"]
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Hcl)
    }

    fn is_dependency_capable(&self) -> bool {
        false
    }

    fn supports_slice(&self) -> bool {
        false
    }
}
