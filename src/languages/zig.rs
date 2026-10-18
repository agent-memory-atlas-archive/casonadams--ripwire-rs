use super::{DialectGroup, LanguageSemantics};
use crate::domain::Lang;

pub struct ZigLanguage;
pub static ZIG_LANG: ZigLanguage = ZigLanguage;

const ZIG_STD_MEMBERS: &[&str] = &[
    "init",
    "deinit",
    "alloc",
    "free",
    "create",
    "destroy",
    "dupe",
    "append",
    "appendSlice",
    "pop",
    "clearRetainingCapacity",
    "clearAndFree",
    "toOwnedSlice",
    "get",
    "put",
    "remove",
    "contains",
    "iterator",
    "next",
    "format",
    "print",
    "read",
    "write",
    "close",
];

const ZIG_STD_QUALIFIERS: &[&str] = &[
    "std",
    "mem",
    "fs",
    "io",
    "os",
    "posix",
    "fmt",
    "math",
    "debug",
    "testing",
    "heap",
    "time",
    "Thread",
    "ArrayList",
    "StringHashMap",
    "AutoHashMap",
];

impl LanguageSemantics for ZigLanguage {
    fn tag(&self) -> &'static str {
        "zig"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Zig)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        ZIG_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        ZIG_STD_QUALIFIERS
    }
}
