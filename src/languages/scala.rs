use super::java::{JVM_STD_MEMBERS, JVM_STD_QUALIFIERS};
use super::{DialectGroup, LanguageSemantics};

pub struct ScalaLanguage;
pub static SCALA_LANG: ScalaLanguage = ScalaLanguage;

impl LanguageSemantics for ScalaLanguage {
    fn tag(&self) -> &'static str {
        "scala"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["scala", "sc"]
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Jvm
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        JVM_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        JVM_STD_QUALIFIERS
    }
}
