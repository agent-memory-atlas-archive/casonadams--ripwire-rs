use super::{
    ComplexityKinds, DefCaptureGateContext, DialectGroup, LanguageSemantics, SyntaxKinds,
    VisibilityMode, resolve_relative_or_exact, strip_import_quotes,
};
use crate::domain::{Lang, SymKind};
use std::collections::HashMap;

pub struct PythonLanguage;
pub static PYTHON_LANG: PythonLanguage = PythonLanguage;

const PYTHON_STD_MEMBERS: &[&str] = &[
    "append",
    "extend",
    "insert",
    "remove",
    "pop",
    "clear",
    "index",
    "count",
    "sort",
    "reverse",
    "copy",
    "get",
    "keys",
    "values",
    "items",
    "update",
    "setdefault",
    "split",
    "rsplit",
    "splitlines",
    "join",
    "strip",
    "lstrip",
    "rstrip",
    "replace",
    "find",
    "rfind",
    "startswith",
    "endswith",
    "lower",
    "upper",
    "format",
    "encode",
    "decode",
    "read",
    "write",
    "readline",
    "readlines",
    "close",
    "flush",
    "add",
    "discard",
    "loads",
    "dumps",
    "load",
    "dump",
    "match",
    "search",
    "sub",
    "findall",
    "compile",
    "exists",
    "is_file",
    "is_dir",
    "mkdir",
    "unlink",
    "resolve",
    "info",
    "debug",
    "warning",
    "error",
    "exception",
];

const PYTHON_STD_QUALIFIERS: &[&str] = &[
    "os",
    "sys",
    "re",
    "json",
    "math",
    "time",
    "datetime",
    "pathlib",
    "Path",
    "itertools",
    "functools",
    "collections",
    "subprocess",
    "logging",
    "typing",
    "shutil",
    "tempfile",
    "hashlib",
    "base64",
    "urllib",
    "asyncio",
    "pytest",
    "unittest",
    "dict",
    "list",
    "set",
    "str",
    "bytes",
];

const PYTHON_ENUM_BASES: &[&str] = &[
    "Enum",
    "IntEnum",
    "StrEnum",
    "Flag",
    "IntFlag",
    "ReprEnum",
    "Choices",
    "TextChoices",
    "IntegerChoices",
];

fn has_enclosing_method_in_class(node: tree_sitter::Node<'_>) -> bool {
    let mut saw_fn = false;
    let mut curr = node.parent();
    while let Some(up) = curr {
        match up.kind() {
            "function_definition" => saw_fn = true,
            "class_definition" => return saw_fn,
            _ => {}
        }
        curr = up.parent();
    }
    false
}

fn is_py_self_field_capture(
    name_node: Option<tree_sitter::Node<'_>>,
    def_node: Option<tree_sitter::Node<'_>>,
    src: &[u8],
) -> bool {
    let (Some(nn), Some(dn)) = (name_node, def_node) else {
        return false;
    };
    let Some(attr) = nn.parent().filter(|p| p.kind() == "attribute") else {
        return false;
    };
    let is_self = attr
        .child_by_field_name("object")
        .is_some_and(|o| o.kind() == "identifier" && o.utf8_text(src) == Ok("self"));
    is_self && has_enclosing_method_in_class(dn)
}

fn is_py_enum_base_node(base: tree_sitter::Node<'_>, src: &[u8]) -> bool {
    let ident = if base.kind() == "attribute" {
        base.child_by_field_name("attribute")
    } else {
        Some(base)
    };
    let Some(node) = ident.filter(|n| n.kind() == "identifier") else {
        return false;
    };
    node.utf8_text(src)
        .is_ok_and(|name| PYTHON_ENUM_BASES.contains(&name))
}

fn is_py_enum_member_target(name_node: Option<tree_sitter::Node<'_>>, src: &[u8]) -> bool {
    let Some(cls) = name_node
        .and_then(|n| n.parent())
        .and_then(|n| n.parent())
        .and_then(|n| n.parent())
        .and_then(|n| n.parent())
        .filter(|c| c.kind() == "class_definition")
    else {
        return false;
    };
    let Some(bases) = cls.child_by_field_name("superclasses") else {
        return false;
    };
    (0..bases.named_child_count())
        .filter_map(|i| bases.named_child(i))
        .any(|b| is_py_enum_base_node(b, src))
}

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &["case_clause"],
    flat: &["elif_clause"],
    nesting: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "except_clause",
        "conditional_expression",
    ],
    nest_only: &["match_statement"],
    else_kinds: &["else_clause"],
    else_bodies: &[],
    jumps: &[
        "break_statement",
        "continue_statement",
        "return_statement",
        "raise_statement",
    ],
    logical_nodes: &[],
    binary_nodes: &["boolean_operator"],
    logical_ops: &["and", "or"],
};

const SYNTAX: SyntaxKinds = SyntaxKinds {
    scope_containers: &["class_definition"],
    member_access: &["attribute"],
    field_access: &["attribute"],
    body_blocks: &["block"],
    body_wrappers: &[
        "function_definition",
        "parenthesized_expression",
        "expression_list",
    ],
    function_defs: &["function_definition"],
    identifiers: &["identifier"],
    type_contexts: &["type_parameter", "generic_type"],
    callable_wrappers: &["parenthesized_expression"],
    binding_patterns: &["tuple_pattern"],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for PythonLanguage {
    fn env_reads(&self) -> &'static [crate::languages::kit::flags::EnvRead] {
        &[crate::languages::kit::flags::EnvRead::Call("getenv"), crate::languages::kit::flags::EnvRead::Call("environ.get"), crate::languages::kit::flags::EnvRead::Index("environ")]
    }

    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn tag(&self) -> &'static str {
        "py"
    }

    fn color(&self) -> Option<&'static str> {
        Some("#3572a5")
    }

    fn lint_return_type<'a>(&self, sig: &'a str, _name: &str) -> Option<&'a str> {
        super::lint_hooks::arrow_return_type(sig)
    }

    fn lint_name_scope<'a>(&self, scope: Option<&'a str>) -> Option<&'a str> {
        scope
    }

    fn is_generated_file_name(&self, file_name: &str) -> bool {
        file_name.ends_with("_pb2.py")
    }
    super::impl_grammar_method!(crate tree_sitter_python::LANGUAGE, "tags.scm");

    fn is_read_capture_allowed(&self, node: tree_sitter::Node<'_>) -> bool {
        python_read_position(node)
    }

    fn read_bindings<'a>(
        &self,
        root: tree_sitter::Node<'_>,
        source: &'a [u8],
    ) -> super::reads::ReadBindings<'a> {
        super::reads::ReadBindings::collect(root, source, |node| {
            python_binding_scope(node).map(|scope| {
                scope
                    .child_by_field_name("body")
                    .unwrap_or(scope)
                    .byte_range()
            })
        })
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py", "pyi"]
    }

    fn test_run_command(
        &self,
        path: &str,
        source: &str,
        _manifest: Option<&str>,
    ) -> Option<String> {
        (source.contains("__name__ == \"__main__\"") || source.contains("__name__ == '__main__'"))
            .then(|| format!("python3 {path}"))
    }

    fn display_color(&self) -> &'static str {
        "#3572a5"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Python)
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        if ctx.kind == SymKind::Field {
            return is_py_self_field_capture(ctx.name_node, ctx.def_node, ctx.source_bytes);
        }
        if ctx.sub == Some("enummember") {
            return is_py_enum_member_target(ctx.name_node, ctx.source_bytes);
        }
        true
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        PYTHON_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        PYTHON_STD_QUALIFIERS
    }

    fn import_binding<'a>(&self, raw_target: &'a str) -> Option<(&'a str, &'a str)> {
        let clean = strip_import_quotes(raw_target);
        let (module, bound) = clean
            .split_once(" as ")
            .map_or((clean, clean), |(m, a)| (m.trim(), a.trim()));
        (!module.is_empty()).then_some((module, bound))
    }

    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        let (module, _) = self.import_binding(raw_target)?;
        let rel_mod = module.trim_start_matches('.').replace('.', "/");
        if rel_mod.is_empty() {
            return None;
        }
        resolve_relative_or_exact(
            importer_path,
            &rel_mod,
            file_index,
            &[".py", "/__init__.py"],
        )
    }
}

fn python_scope(kind: &str) -> bool {
    matches!(
        kind,
        "function_definition"
            | "lambda"
            | "class_definition"
            | "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension"
            | "generator_expression"
    )
}

fn python_binding_target(parent: tree_sitter::Node<'_>, node: tree_sitter::Node<'_>) -> bool {
    let field = match parent.kind() {
        "assignment" | "augmented_assignment" | "for_statement" | "for_in_clause" => "left",
        "named_expression"
        | "function_definition"
        | "class_definition"
        | "default_parameter"
        | "typed_default_parameter" => "name",
        "aliased_import" => "alias",
        "parameters" | "lambda_parameters" => return node.parent() == Some(parent),
        "typed_parameter" => return parent.named_child(0) == Some(node),
        "import_statement" => return true,
        "import_from_statement" => return !super::reads::in_field(parent, node, "module_name"),
        "list_splat_pattern" | "dictionary_splat_pattern" | "as_pattern_target" => return true,
        _ => return false,
    };
    super::reads::in_field(parent, node, field)
}

fn python_binding_scope(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    if node.kind() != "identifier" {
        return None;
    }
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        if python_binding_target(parent, node) {
            let owner = if matches!(parent.kind(), "function_definition" | "class_definition") {
                parent
            } else {
                node
            };
            return super::reads::enclosing_scope(owner, python_scope)
                .filter(|scope| scope.kind() != "class_definition");
        }
        if python_scope(parent.kind()) {
            break;
        }
        ancestor = parent.parent();
    }
    None
}

fn python_non_value(parent: tree_sitter::Node<'_>, node: tree_sitter::Node<'_>) -> bool {
    if python_binding_target(parent, node) {
        return true;
    }
    match parent.kind() {
        "type"
        | "import_statement"
        | "import_from_statement"
        | "global_statement"
        | "nonlocal_statement"
        | "dotted_name"
        | "pattern_list"
        | "case_pattern" => true,
        "call" => parent.child_by_field_name("function") == Some(node),
        "attribute" => parent.child_by_field_name("attribute") == Some(node),
        "keyword_argument" => super::reads::in_field(parent, node, "name"),
        _ => false,
    }
}

fn python_read_position(node: tree_sitter::Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    // Class namespaces are not lexical parents of method bodies. Class-body lookup is not
    // resolved by these bare non-local captures.
    if super::reads::enclosing_scope(node, python_scope)
        .is_some_and(|scope| scope.kind() == "class_definition")
    {
        return false;
    }
    let expression = matches!(
        parent.kind(),
        "return_statement"
            | "binary_operator"
            | "boolean_operator"
            | "comparison_operator"
            | "unary_operator"
            | "not_operator"
            | "argument_list"
            | "list"
            | "tuple"
            | "set"
            | "dictionary"
            | "pair"
            | "subscript"
            | "conditional_expression"
            | "await"
            | "yield"
            | "expression_statement"
            | "assignment"
            | "named_expression"
            | "keyword_argument"
            | "attribute"
            | "if_statement"
            | "while_statement"
            | "default_parameter"
            | "typed_default_parameter"
            | "interpolation"
            | "for_statement"
            | "for_in_clause"
            | "parenthesized_expression"
    );
    expression && super::reads::value_position(node, python_non_value, python_scope)
}
