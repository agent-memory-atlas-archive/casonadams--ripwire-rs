use super::{DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::Lang;

pub struct SwiftLanguage;
pub static SWIFT_LANG: SwiftLanguage = SwiftLanguage;

const SWIFT_STD_MEMBERS: &[&str] = &[
    "append",
    "insert",
    "remove",
    "removeAll",
    "contains",
    "map",
    "compactMap",
    "flatMap",
    "filter",
    "reduce",
    "forEach",
    "first",
    "last",
    "sorted",
    "reversed",
    "split",
    "joined",
    "lowercased",
    "uppercased",
    "hasPrefix",
    "hasSuffix",
];

const SWIFT_STD_QUALIFIERS: &[&str] = &[
    "String",
    "Int",
    "Double",
    "Bool",
    "Array",
    "Dictionary",
    "Set",
    "Date",
    "URL",
    "Data",
    "UUID",
    "FileManager",
    "UserDefaults",
    "NotificationCenter",
    "DispatchQueue",
    "Task",
    "MainActor",
    "JSONEncoder",
    "JSONDecoder",
];

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

    fn std_member_names(&self) -> &'static [&'static str] {
        SWIFT_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        SWIFT_STD_QUALIFIERS
    }
}
