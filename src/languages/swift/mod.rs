use super::{ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds, VisibilityMode};
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

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &["switch_entry", "guard_statement"],
    flat: &[],
    nesting: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "repeat_while_statement",
        "catch_block",
        "ternary_expression",
    ],
    nest_only: &["switch_statement"],
    else_kinds: &["else"],
    else_bodies: &[],
    jumps: &["control_transfer_statement"],
    logical_nodes: &[
        "conjunction_expression",
        "disjunction_expression",
        "nil_coalescing_expression",
    ],
    binary_nodes: &[],
    logical_ops: &[],
};

const SYNTAX: SyntaxKinds = SyntaxKinds {
    scope_containers: &["class_declaration", "protocol_declaration"],
    member_access: &["navigation_expression", "navigation_suffix"],
    body_blocks: &[
        "function_body",
        "class_body",
        "enum_class_body",
        "protocol_body",
    ],
    body_wrappers: &["property_declaration", "as_expression"],
    function_defs: &["function_declaration"],
    non_local_declarations: &["function_declaration", "class_declaration"],
    identifiers: &["identifier", "simple_identifier"],
    type_contexts: &[
        "type_parameters",
        "type_parameter",
        "type_annotation",
        "type_arguments",
    ],
    callable_wrappers: &["as_expression"],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for SwiftLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn member_receiver<'a>(&self, access: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
        if access.kind() == "navigation_suffix" {
            access.parent().and_then(|nav| nav.named_child(0))
        } else {
            None
        }
    }

    fn tag(&self) -> &'static str {
        "swift"
    }

    fn color(&self) -> Option<&'static str> {
        Some("#f05138")
    }
    super::impl_grammar_method!(crate tree_sitter_swift::LANGUAGE, "tags.scm");

    fn lint_return_type<'a>(&self, sig: &'a str, _name: &str) -> Option<&'a str> {
        super::lint_hooks::arrow_return_type(sig)
    }

    fn type_declaration_keywords(&self) -> &'static [&'static str] {
        &["typealias", "associatedtype"]
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
