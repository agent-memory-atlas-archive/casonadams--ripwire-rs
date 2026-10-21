use super::c::{C_FAMILY_KEYWORDS, CPP_STD_MEMBERS, CPP_STD_QUALIFIERS, resolve_c_family_include};
use super::{ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds};
use std::collections::HashMap;

pub struct ObjCLanguage;
pub static OBJC_LANG: ObjCLanguage = ObjCLanguage;

const OBJC_FOUNDATION_MEMBERS: &[&str] = &[
    "alloc",
    "init",
    "new",
    "copy",
    "mutableCopy",
    "description",
    "hash",
    "isEqual",
    "count",
    "objectForKey",
    "setObject",
    "addObject",
    "removeObject",
    "removeAllObjects",
    "containsObject",
    "firstObject",
    "lastObject",
    "stringWithFormat",
    "UTF8String",
    "length",
    "boolValue",
    "intValue",
    "integerValue",
    "doubleValue",
];

const OBJC_FOUNDATION_QUALIFIERS: &[&str] = &[
    "NSObject",
    "NSString",
    "NSMutableString",
    "NSArray",
    "NSMutableArray",
    "NSDictionary",
    "NSMutableDictionary",
    "NSSet",
    "NSMutableSet",
    "NSNumber",
    "NSData",
    "NSMutableData",
    "NSURL",
    "NSURLRequest",
    "NSURLSession",
    "NSFileManager",
    "NSBundle",
    "NSThread",
    "NSNotificationCenter",
    "NSUserDefaults",
    "NSDate",
    "NSError",
    "NSJSONSerialization",
];

const KINDS: ComplexityKinds = ComplexityKinds {
    nesting: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "do_statement",
        "catch_clause",
        "conditional_expression",
    ],
    ..ComplexityKinds::C_STYLE
};

const SYNTAX: SyntaxKinds = SyntaxKinds {
    scope_containers: &[
        "class_declaration",
        "class_interface",
        "class_implementation",
        "struct_specifier",
        "struct_declaration",
        "protocol_declaration",
    ],
    member_access: &["field_expression"],
    field_access: &["field_expression"],
    body_blocks: &[
        "compound_statement",
        "declaration_list",
        "field_declaration_list",
        "implementation_definition",
    ],
    body_wrappers: &["property_declaration", "parenthesized_expression"],
    function_defs: &["function_definition", "method_definition"],
    function_declarators: &["function_declarator"],
    non_local_declarations: &["class_declaration"],
    identifiers: &["identifier"],
    callables: &["function_declarator"],
    callable_wrappers: &["parenthesized_expression"],
    signature_holders: &["declaration"],
    binding_patterns: &[
        "pointer_declarator",
        "array_declarator",
        "parenthesized_declarator",
    ],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for ObjCLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }
    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }
    fn tag(&self) -> &'static str {
        "m"
    }
    super::impl_grammar_method!(crate tree_sitter_objc::LANGUAGE, "../queries/objc/tags.scm");

    fn lint_tag(&self) -> &'static str {
        "objc"
    }

    fn lint_return_type<'a>(&self, sig: &'a str, name: &str) -> Option<&'a str> {
        super::lint_hooks::prefix_return_type(sig, name)
    }

    fn type_declaration_keywords(&self) -> &'static [&'static str] {
        &["typedef", "enum"]
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["m", "mm"]
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::CFamily
    }

    fn supports_c_layout(&self) -> bool {
        true
    }

    fn is_builtin_keyword(&self, name: &str) -> bool {
        C_FAMILY_KEYWORDS.contains(&name)
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        CPP_STD_MEMBERS.contains(&name) || OBJC_FOUNDATION_MEMBERS.contains(&name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        CPP_STD_QUALIFIERS.contains(&qualifier) || OBJC_FOUNDATION_QUALIFIERS.contains(&qualifier)
    }

    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        resolve_c_family_include(importer_path, raw_target, file_index)
    }
}
