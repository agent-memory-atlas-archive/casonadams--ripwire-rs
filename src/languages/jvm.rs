use super::{DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::Lang;

pub struct JavaLanguage;
pub struct KotlinLanguage;
pub struct ScalaLanguage;
pub struct CSharpLanguage;
pub struct SwiftLanguage;
pub struct PhpLanguage;

pub static JAVA_LANG: JavaLanguage = JavaLanguage;
pub static KOTLIN_LANG: KotlinLanguage = KotlinLanguage;
pub static SCALA_LANG: ScalaLanguage = ScalaLanguage;
pub static CSHARP_LANG: CSharpLanguage = CSharpLanguage;
pub static SWIFT_LANG: SwiftLanguage = SwiftLanguage;
pub static PHP_LANG: PhpLanguage = PhpLanguage;

impl LanguageSemantics for JavaLanguage {
    fn tag(&self) -> &'static str {
        "java"
    }

    fn display_color(&self) -> &'static str {
        "#b07219"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Jvm
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn is_entrypoint_symbol(
        &self,
        sym_name: &str,
        _scope: Option<&str>,
        kind: crate::domain::SymKind,
        rel_path: &str,
    ) -> bool {
        kind.is_callable() && !super::is_non_entrypoint_path(rel_path) && sym_name == "main"
    }
}

impl LanguageSemantics for KotlinLanguage {
    fn tag(&self) -> &'static str {
        "kt"
    }

    fn display_color(&self) -> &'static str {
        "#a97bff"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Jvm
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }
}

impl LanguageSemantics for ScalaLanguage {
    fn tag(&self) -> &'static str {
        "scala"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Jvm
    }
}

impl LanguageSemantics for CSharpLanguage {
    fn tag(&self) -> &'static str {
        "cs"
    }

    fn display_color(&self) -> &'static str {
        "#178600"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::CSharp)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn is_entrypoint_symbol(
        &self,
        sym_name: &str,
        _scope: Option<&str>,
        kind: crate::domain::SymKind,
        rel_path: &str,
    ) -> bool {
        kind.is_callable()
            && !super::is_non_entrypoint_path(rel_path)
            && (sym_name == "main" || sym_name == "Main")
    }
}

impl LanguageSemantics for SwiftLanguage {
    fn tag(&self) -> &'static str {
        "swift"
    }

    fn display_color(&self) -> &'static str {
        "#f05138"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Swift)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }
}

impl LanguageSemantics for PhpLanguage {
    fn tag(&self) -> &'static str {
        "php"
    }

    fn display_color(&self) -> &'static str {
        "#4f5d95"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Php)
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }
}
