use super::{ComplexityKinds, DialectGroup, LanguageSemantics};
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

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &["switch_case"],
    flat: &[],
    nesting: &[
        "if_expression",
        "if_statement",
        "for_expression",
        "for_statement",
        "while_expression",
        "while_statement",
        "catch_expression",
    ],
    nest_only: &["switch_expression"],
    else_kinds: &["else_clause"],
    else_bodies: &[],
    jumps: &[
        "break_expression",
        "continue_expression",
        "return_expression",
    ],
    logical_nodes: &[],
    binary_nodes: &["binary_expression"],
    logical_ops: &["and", "or", "orelse"],
};

impl LanguageSemantics for ZigLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }

    fn tag(&self) -> &'static str {
        "zig"
    }
    super::impl_grammar_method!(crate tree_sitter_zig::LANGUAGE, "../queries/zig/tags.scm");

    fn extensions(&self) -> &'static [&'static str] {
        &["zig", "zon"]
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
