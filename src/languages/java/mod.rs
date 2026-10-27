use super::{
    ComplexityKinds, DefCaptureGateContext, DialectGroup, LanguageSemantics, SyntaxKinds,
    VisibilityMode,
};

pub struct JavaLanguage;
pub static JAVA_LANG: JavaLanguage = JavaLanguage;

pub(super) const JVM_STD_MEMBERS: &[&str] = &[
    "get",
    "set",
    "put",
    "remove",
    "add",
    "addAll",
    "clear",
    "contains",
    "containsKey",
    "size",
    "isEmpty",
    "iterator",
    "stream",
    "map",
    "filter",
    "flatMap",
    "forEach",
    "collect",
    "toList",
    "toArray",
    "orElse",
    "orElseGet",
    "orElseThrow",
    "isPresent",
    "equals",
    "hashCode",
    "toString",
    "compareTo",
    "length",
    "charAt",
    "substring",
    "split",
    "trim",
    "replace",
    "startsWith",
    "endsWith",
    "toLowerCase",
    "toUpperCase",
    "format",
    "valueOf",
    "parseInt",
    "parseLong",
    "append",
];

pub(super) const JVM_STD_QUALIFIERS: &[&str] = &[
    "System",
    "Math",
    "String",
    "Integer",
    "Long",
    "Double",
    "Boolean",
    "Objects",
    "Collections",
    "Arrays",
    "Optional",
    "Stream",
    "Collectors",
    "List",
    "Map",
    "Set",
    "Files",
    "Paths",
    "Path",
    "UUID",
    "Instant",
    "Duration",
    "LocalDate",
    "LocalDateTime",
    "CompletableFuture",
    "Thread",
    "Executors",
    "LoggerFactory",
];

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &["switch_block_statement_group", "switch_rule"],
    flat: &[],
    nesting: &[
        "if_statement",
        "for_statement",
        "enhanced_for_statement",
        "while_statement",
        "do_statement",
        "catch_clause",
        "ternary_expression",
    ],
    nest_only: &["switch_expression"],
    else_kinds: &["else"],
    else_bodies: &[],
    jumps: &[
        "break_statement",
        "continue_statement",
        "return_statement",
        "throw_statement",
        "yield_statement",
    ],
    logical_nodes: &[],
    binary_nodes: &["binary_expression"],
    logical_ops: &["&&", "||"],
};

const SYNTAX: SyntaxKinds = SyntaxKinds {
    scope_containers: &[
        "class_declaration",
        "record_declaration",
        "enum_declaration",
        "interface_declaration",
    ],
    scoped_paths: &["scoped_identifier"],
    member_access: &["method_invocation"],
    body_blocks: &["block", "class_body"],
    body_wrappers: &["variable_declarator", "parenthesized_expression"],
    non_local_declarations: &[
        "class_declaration",
        "interface_declaration",
        "enum_declaration",
    ],
    identifiers: &["identifier"],
    type_contexts: &[
        "type_parameters",
        "type_parameter",
        "type_arguments",
        "generic_type",
    ],
    callable_wrappers: &["variable_declarator", "parenthesized_expression"],
    ..SyntaxKinds::NONE
};

const TYPED: super::typed::TypedSyntax = super::typed::TypedSyntax {
    classes: &[
        "class_declaration",
        "interface_declaration",
        "enum_declaration",
        "record_declaration",
    ],
    supertype_clauses: &["superclass", "super_interfaces", "extends_interfaces"],
    params: &["formal_parameter", "spread_parameter"],
    functions: &["method_declaration", "constructor_declaration", "lambda_expression"],
    locals: &["local_variable_declaration"],
    fields: &["field_declaration"],
    declarators: &["variable_declarator"],
    blocks: &["block", "constructor_body"],
    constructions: &["object_creation_expression"],
    inferred: &["var"],
    assignments: &[],
    calls: &[("method_invocation", "name")],
    return_type_field: "type",
    superclass: Some(("superclass", "super")),
    declaration_name: None,
    class_params: &[],
};

impl LanguageSemantics for JavaLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }
    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }
    super::impl_typed_class_hooks!(TYPED);
    fn overloads_by_arity(&self) -> bool {
        true
    }
    fn tag(&self) -> &'static str {
        "java"
    }

    fn lint_groups(&self) -> &'static [crate::languages::LintGroup] {
        use crate::languages::LintGroup as G;
        &[G::Cast, G::DoWhile, G::Parens, G::SelfAssign, G::Unreachable, G::Naming]
    }
    super::impl_grammar_method!(crate tree_sitter_java::LANGUAGE, "tags.scm");

    fn lint_return_type<'a>(&self, sig: &'a str, name: &str) -> Option<&'a str> {
        super::lint_hooks::prefix_return_type(sig, name)
    }

    fn type_declaration_keywords(&self) -> &'static [&'static str] {
        &["enum"]
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

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        ctx.sub != Some("constant") || super::is_screaming_snake(ctx.name)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        JVM_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        JVM_STD_QUALIFIERS
    }
}
