mod callables;

use super::{
    ComplexityKinds, DialectGroup, LanguageSemantics, SyntaxKinds, VisibilityMode,
    normalize_lexical_path, probe_with_suffixes, strip_import_quotes,
};
use crate::domain::{Lang, SymKind};
use std::collections::HashMap;
use std::path::Path;

pub struct RustLanguage;
pub static RUST_LANG: RustLanguage = RustLanguage;

const RUST_BUILTIN_MACROS: &[&str] = &[
    "matches",
    "write",
    "writeln",
    "print",
    "println",
    "eprint",
    "eprintln",
    "format",
    "format_args",
    "vec",
    "panic",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "todo",
    "unimplemented",
    "unreachable",
    "cfg",
    "env",
    "option_env",
    "include",
    "include_str",
    "include_bytes",
    "concat",
    "stringify",
    "line",
    "column",
    "file",
    "module_path",
    "compile_error",
    "thread_local",
];

const RUST_STD_MEMBERS: &[&str] = &[
    "len",
    "is_empty",
    "push",
    "push_str",
    "pop",
    "clear",
    "get",
    "get_mut",
    "insert",
    "remove",
    "contains",
    "contains_key",
    "iter",
    "iter_mut",
    "into_iter",
    "as_slice",
    "as_mut_slice",
    "as_str",
    "as_bytes",
    "as_ref",
    "as_mut",
    "as_deref",
    "as_deref_mut",
    "clone",
    "to_string",
    "to_owned",
    "to_vec",
    "unwrap",
    "unwrap_or",
    "unwrap_or_default",
    "unwrap_or_else",
    "expect",
    "ok",
    "err",
    "map",
    "and_then",
    "or_else",
    "filter",
    "collect",
    "extend",
    "reserve",
    "with_capacity",
    "default",
    "from",
    "into",
    "fmt",
    "eq",
    "cmp",
    "partial_cmp",
    "hash",
    "index",
    "deref",
    "next",
];

const RUST_STD_QUALIFIERS: &[&str] = &[
    "std",
    "core",
    "alloc",
    "Vec",
    "String",
    "Option",
    "Result",
    "Box",
    "Arc",
    "Rc",
    "Mutex",
    "RwLock",
    "HashMap",
    "HashSet",
    "BTreeMap",
    "BTreeSet",
    "VecDeque",
    "Path",
    "PathBuf",
    "OsStr",
    "OsString",
    "CStr",
    "CString",
    "File",
    "Command",
    "Instant",
    "Duration",
    "SystemTime",
    "env",
    "fs",
    "io",
    "mem",
    "ptr",
    "slice",
    "str",
    "fmt",
    "iter",
    "cmp",
    "ops",
    "sync",
    "thread",
];

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &["match_arm"],
    flat: &[],
    nesting: &[
        "if_expression",
        "for_expression",
        "while_expression",
        "loop_expression",
    ],
    nest_only: &["match_expression"],
    else_kinds: &["else_clause"],
    else_bodies: &[],
    jumps: &[
        "break_expression",
        "continue_expression",
        "return_expression",
    ],
    logical_nodes: &[],
    binary_nodes: &["binary_expression"],
    logical_ops: &["&&", "||"],
};

const SYNTAX: SyntaxKinds = SyntaxKinds {
    scope_containers: &["mod_item", "trait_item"],
    scoped_paths: &["scoped_identifier", "scoped_type_identifier"],
    member_access: &["field_expression"],
    field_access: &["field_expression"],
    body_blocks: &["block", "declaration_list", "field_declaration_list"],
    body_wrappers: &["parenthesized_expression"],
    function_defs: &["function_item"],
    local_bindings: &["let_condition"],
    identifiers: &["identifier"],
    type_contexts: &[
        "type_parameters",
        "type_parameter",
        "type_arguments",
        "generic_type",
    ],
    callable_wrappers: &["parenthesized_expression"],
    binding_patterns: &[
        "tuple_pattern",
        "slice_pattern",
        "tuple_struct_pattern",
        "struct_pattern",
        "field_pattern",
        "mut_pattern",
        "ref_pattern",
    ],
    ..SyntaxKinds::NONE
};

impl LanguageSemantics for RustLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }

    fn syntax_kinds(&self) -> &'static SyntaxKinds {
        &SYNTAX
    }

    fn extract_custom_scope(
        &self,
        parent: tree_sitter::Node<'_>,
        _name_node: tree_sitter::Node<'_>,
        source_bytes: &[u8],
    ) -> Option<String> {
        if parent.kind() != "impl_item" {
            return None;
        }
        let ty = parent.child_by_field_name("type")?;
        let text = ty.utf8_text(source_bytes).ok()?;
        let name = text.split('<').next().unwrap_or(text).trim();
        (!name.is_empty()).then(|| name.to_string())
    }

    fn tag(&self) -> &'static str {
        "rs"
    }

    fn color(&self) -> Option<&'static str> {
        Some("#dea584")
    }

    fn lint_return_type<'a>(&self, sig: &'a str, _name: &str) -> Option<&'a str> {
        super::lint_hooks::arrow_return_type(sig)
    }

    fn lint_name_scope<'a>(&self, scope: Option<&'a str>) -> Option<&'a str> {
        scope
    }

    fn type_declaration_keywords(&self) -> &'static [&'static str] {
        &["type"]
    }

    fn is_test_module_stem(&self, file_stem: &str) -> bool {
        let lower = file_stem.to_ascii_lowercase();
        matches!(lower.as_str(), "tests" | "test") || lower.ends_with("_tests")
    }

    super::impl_grammar_method!(crate tree_sitter_rust::LANGUAGE, "tags.scm");

    fn is_read_capture_allowed(&self, node: tree_sitter::Node<'_>) -> bool {
        rust_read_position(node)
    }

    fn callable_bindings(
        &self,
        root: tree_sitter::Node<'_>,
        source: &[u8],
    ) -> Vec<super::CallableBinding> {
        callables::collect(root, source)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        let root = qualifier.split("::").next().unwrap_or(qualifier);
        matches!(root, "std" | "core" | "alloc") || RUST_STD_QUALIFIERS.contains(&qualifier)
    }

    fn implementations(
        &self,
        root: tree_sitter::Node<'_>,
        source: &[u8],
    ) -> Vec<super::Implementation> {
        collect_rust_implementations(root, source)
    }

    fn read_bindings<'a>(
        &self,
        root: tree_sitter::Node<'_>,
        source: &'a [u8],
    ) -> super::reads::ReadBindings<'a> {
        collect_rust_read_bindings(root, source)
    }

    fn has_lifetime_tick_syntax(&self) -> bool {
        true
    }

    fn display_color(&self) -> &'static str {
        "#dea584"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Rust)
    }

    fn has_trait_impls(&self) -> bool {
        true
    }

    fn self_receivers(&self) -> &'static [&'static str] {
        &["Self"]
    }

    fn keeps_path_qualifier(&self) -> bool {
        true
    }

    fn crate_root_suffixes(&self) -> &'static [&'static str] {
        &["src/lib.rs", "src/main.rs"]
    }

    fn is_callable_field_access(&self, name_node: tree_sitter::Node<'_>) -> bool {
        let kinds: Vec<&str> = std::iter::successors(name_node.parent(), |n| n.parent())
            .take(3)
            .map(|n| n.kind())
            .collect();
        kinds
            == [
                "field_expression",
                "parenthesized_expression",
                "call_expression",
            ]
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn should_ignore_call(&self, name_node: Option<tree_sitter::Node<'_>>, name: &str) -> bool {
        let is_macro_inv = name_node
            .and_then(|n| n.parent())
            .is_some_and(|p| p.kind() == "macro_invocation");
        is_macro_inv && RUST_BUILTIN_MACROS.contains(&name)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        RUST_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        RUST_STD_QUALIFIERS
    }

    fn symbol_implied_import(&self, kind: SymKind, name: &str) -> Option<String> {
        (kind == SymKind::Section).then(|| format!("self::{name}"))
    }
    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        resolve_rust_use_target(importer_path, raw_target, file_index)
    }
}

fn find_rust_crate_src_dir(importer_path: &str) -> &str {
    if let Some(pos) = importer_path.rfind("/src/") {
        &importer_path[..pos + 4]
    } else if importer_path.starts_with("src/") {
        "src"
    } else {
        Path::new(importer_path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
    }
}

fn resolve_rust_super_dir(importer_path: &str) -> &str {
    let p = Path::new(importer_path);
    let is_mod_root = matches!(
        p.file_name().and_then(|n| n.to_str()),
        Some("mod.rs" | "lib.rs" | "main.rs")
    );
    let dir = if is_mod_root {
        p.parent().and_then(|d| d.parent())
    } else {
        p.parent()
    };
    dir.and_then(|d| d.to_str()).unwrap_or("")
}

fn split_rust_anchor(clean: &str) -> Option<(&str, &str)> {
    for anchor in ["crate", "self", "super"] {
        if clean == anchor {
            return Some((anchor, ""));
        }
        if let Some(rest) = clean
            .strip_prefix(anchor)
            .and_then(|s| s.strip_prefix("::"))
        {
            return Some((anchor, rest));
        }
    }
    None
}

fn resolve_rust_base_and_rest<'a>(
    importer_path: &'a str,
    clean: &'a str,
) -> Option<(&'a str, &'a str)> {
    let (anchor, rest) = split_rust_anchor(clean)?;
    let base = match anchor {
        "crate" => find_rust_crate_src_dir(importer_path),
        "self" => Path::new(importer_path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or(""),
        _ => resolve_rust_super_dir(importer_path),
    };
    Some((base, rest))
}

fn probe_rust_segs_in_base(
    base_dir: &str,
    segs: &[&str],
    file_index: &HashMap<&str, u32>,
) -> Option<u32> {
    for take_len in [segs.len(), segs.len().saturating_sub(1)] {
        if take_len == 0 {
            continue;
        }
        let rel = format!("./{}", segs[..take_len].join("/"));
        let cand = normalize_lexical_path(base_dir, &rel);
        if let Some(id) = probe_with_suffixes(&cand, file_index, &[".rs", "/mod.rs"]) {
            return Some(id);
        }
    }
    None
}

fn resolve_rust_workspace_crate_target(
    clean: &str,
    file_index: &HashMap<&str, u32>,
) -> Option<u32> {
    let segs: Vec<&str> = clean.split("::").filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 {
        return None;
    }
    let crate_dir = format!("crates/{}/src", segs[0].replace('_', "-"));
    probe_rust_segs_in_base(&crate_dir, &segs[1..], file_index)
}

fn resolve_rust_use_target(
    importer_path: &str,
    raw_target: &str,
    file_index: &HashMap<&str, u32>,
) -> Option<u32> {
    let clean = strip_import_quotes(raw_target)
        .split('{')
        .next()
        .unwrap_or("")
        .trim_end_matches("::*")
        .trim_end_matches("::");
    let segs: Vec<&str> = clean.split("::").filter(|s| !s.is_empty()).collect();
    if let Some((base_dir, rest)) = resolve_rust_base_and_rest(importer_path, clean) {
        let rest_segs: Vec<&str> = rest.split("::").filter(|s| !s.is_empty()).collect();
        return probe_rust_segs_in_base(base_dir, &rest_segs, file_index);
    }
    let self_dir = Path::new(importer_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    for base in [self_dir, find_rust_crate_src_dir(importer_path)] {
        if let Some(id) = probe_rust_segs_in_base(base, &segs, file_index) {
            return Some(id);
        }
    }
    resolve_rust_workspace_crate_target(clean, file_index)
}

fn rust_scope(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "closure_expression"
            | "block"
            | "match_arm"
            | "for_expression"
            | "if_expression"
            | "while_expression"
    )
}

fn rust_binding_target(parent: tree_sitter::Node<'_>, node: tree_sitter::Node<'_>) -> bool {
    let field = match parent.kind() {
        "let_declaration" | "let_condition" | "parameter" | "for_expression" => "pattern",
        "const_item" | "static_item" | "function_item" => "name",
        "closure_parameters" => return true,
        "match_pattern" => return true,
        _ => return false,
    };
    super::reads::in_field(parent, node, field)
}

fn rust_binding_scope(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    if node.kind() != "identifier" {
        return None;
    }
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        if rust_binding_target(parent, node) {
            let owner = if matches!(
                parent.kind(),
                "const_item" | "static_item" | "function_item"
            ) {
                parent
            } else {
                node
            };
            return super::reads::enclosing_scope(owner, rust_scope);
        }
        if rust_scope(parent.kind()) {
            break;
        }
        ancestor = parent.parent();
    }
    None
}

fn rust_non_value(parent: tree_sitter::Node<'_>, node: tree_sitter::Node<'_>) -> bool {
    if rust_binding_target(parent, node) {
        return true;
    }
    match parent.kind() {
        "type_arguments"
        | "type_parameters"
        | "use_declaration"
        | "macro_invocation"
        | "token_tree"
        | "attribute_item"
        | "inner_attribute_item" => true,
        "call_expression" => parent.child_by_field_name("function") == Some(node),
        "assignment_expression" | "compound_assignment_expr" => {
            super::reads::in_field(parent, node, "left")
        }
        "scoped_identifier" => parent.child_by_field_name("path") == Some(node),
        _ => false,
    }
}

fn rust_read_position(node: tree_sitter::Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    let expression = matches!(
        parent.kind(),
        "binary_expression"
            | "return_expression"
            | "arguments"
            | "array_expression"
            | "tuple_expression"
            | "reference_expression"
            | "unary_expression"
            | "index_expression"
            | "field_expression"
            | "let_declaration"
            | "const_item"
            | "static_item"
            | "assignment_expression"
            | "compound_assignment_expr"
            | "if_expression"
            | "while_expression"
            | "match_expression"
            | "closure_expression"
            | "parenthesized_expression"
            | "expression_statement"
            | "block"
            | "field_initializer"
            | "shorthand_field_initializer"
            | "scoped_identifier"
    );
    expression && super::reads::value_position(node, rust_non_value, rust_scope)
}

fn rust_binding_range(node: tree_sitter::Node<'_>) -> Option<std::ops::Range<usize>> {
    let scope = rust_binding_scope(node)?;
    let mut span = scope.byte_range();
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        if parent.kind() == "let_declaration" {
            span.start = parent.end_byte();
            break;
        }
        if parent == scope {
            break;
        }
        ancestor = parent.parent();
    }
    Some(span)
}

#[derive(Clone, PartialEq)]
enum NameBinding {
    Local,
    Import(String),
}

/// Type-name bindings keyed by the AST id of the container that holds the declaration.
#[derive(Default)]
struct LexicalNames {
    scopes: std::collections::HashMap<usize, std::collections::HashMap<String, NameBinding>>,
}

impl LexicalNames {
    fn bind(&mut self, container: tree_sitter::Node<'_>, name: &str, binding: NameBinding) {
        let names = self.scopes.entry(container.id()).or_default();
        match names.entry(name.to_string()) {
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(binding);
            }
            std::collections::hash_map::Entry::Occupied(mut slot) => {
                if *slot.get() != binding {
                    *slot.get_mut() = NameBinding::Local;
                }
            }
        }
    }

    /// The binding of `name` in the nearest enclosing container that declares it.
    fn lookup(&self, at: tree_sitter::Node<'_>, name: &str) -> Option<&NameBinding> {
        let mut curr = Some(at);
        while let Some(n) = curr {
            if let Some(binding) = self.scopes.get(&n.id()).and_then(|names| names.get(name)) {
                return Some(binding);
            }
            curr = n.parent();
        }
        None
    }
}

fn field_text<'s>(node: tree_sitter::Node<'_>, field: &str, source: &'s [u8]) -> Option<&'s str> {
    node.child_by_field_name(field)?
        .utf8_text(source)
        .ok()
        .map(str::trim)
}

fn join_path(prefix: &str, segment: &str) -> String {
    match (prefix.is_empty(), segment.is_empty()) {
        (_, true) => prefix.to_string(),
        (true, false) => segment.to_string(),
        (false, false) => format!("{prefix}::{segment}"),
    }
}

/// Collects `(bound name, target path)` pairs from a use tree; globs bind nothing.
fn collect_use_tree(
    tree: tree_sitter::Node<'_>,
    prefix: &str,
    source: &[u8],
    out: &mut Vec<(String, String)>,
) {
    match tree.kind() {
        "identifier" | "scoped_identifier" | "crate" | "super" => {
            if let Ok(text) = tree.utf8_text(source) {
                let text = text.trim().trim_start_matches("::");
                let bound = text.rsplit("::").next().unwrap_or_default().to_string();
                out.push((bound, join_path(prefix, text)));
            }
        }
        "self" => {
            if let Some(last) = prefix.rsplit("::").next().filter(|s| !s.is_empty()) {
                out.push((last.to_string(), prefix.to_string()));
            }
        }
        "use_as_clause" => {
            if let (Some(alias), Some(path)) = (
                field_text(tree, "alias", source),
                field_text(tree, "path", source),
            ) && alias != "_"
            {
                let path = path.trim_start_matches("::");
                out.push((alias.to_string(), join_path(prefix, path)));
            }
        }
        "scoped_use_list" => {
            let base = field_text(tree, "path", source).unwrap_or_default();
            let next = join_path(prefix, base.trim_start_matches("::"));
            if let Some(list) = tree.child_by_field_name("list") {
                collect_use_tree(list, &next, source, out);
            }
        }
        "use_list" => {
            let mut cursor = tree.walk();
            for child in tree.named_children(&mut cursor) {
                collect_use_tree(child, prefix, source, out);
            }
        }
        _ => {}
    }
}

fn collect_lexical_names(root: tree_sitter::Node<'_>, source: &[u8]) -> LexicalNames {
    let mut names = LexicalNames::default();
    for node in super::reads::nodes(root) {
        let Some(container) = node.parent() else {
            continue;
        };
        if container
            .parent()
            .is_some_and(|p| matches!(p.kind(), "impl_item" | "trait_item"))
        {
            continue;
        }
        match node.kind() {
            "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item" => {
                if let Some(name) = field_text(node, "name", source) {
                    names.bind(container, name, NameBinding::Local);
                }
            }
            "use_declaration" => {
                let mut imports = Vec::new();
                if let Some(arg) = node.child_by_field_name("argument") {
                    collect_use_tree(arg, "", source, &mut imports);
                }
                for (name, target) in imports {
                    names.bind(container, &name, NameBinding::Import(target));
                }
            }
            _ => {}
        }
    }
    names
}

const WRAPPER_PATHS: &[&str] = &[
    "std::boxed::Box",
    "alloc::boxed::Box",
    "std::sync::Arc",
    "alloc::sync::Arc",
    "std::rc::Rc",
    "alloc::rc::Rc",
];

fn is_unit_struct(node: tree_sitter::Node<'_>) -> bool {
    let mut cursor = node.walk();
    !node.children(&mut cursor).any(|child| {
        matches!(
            child.kind(),
            "field_declaration_list" | "ordered_field_declaration_list"
        )
    })
}

struct AliasDecl<'t> {
    count: usize,
    value: Option<tree_sitter::Node<'t>>,
}

#[derive(Default)]
struct TypeDecls<'s, 't> {
    nominal: std::collections::HashSet<&'s str>,
    unit: std::collections::HashSet<&'s str>,
    aliases: std::collections::HashMap<&'s str, AliasDecl<'t>>,
}

fn collect_type_decls<'s, 't>(root: tree_sitter::Node<'t>, source: &'s [u8]) -> TypeDecls<'s, 't> {
    let mut decls = TypeDecls::default();
    for node in super::reads::nodes(root) {
        if !matches!(
            node.kind(),
            "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item"
        ) {
            continue;
        }
        let Some(name) = field_text(node, "name", source) else {
            continue;
        };
        match node.kind() {
            "struct_item" => {
                if is_unit_struct(node) {
                    decls.unit.insert(name);
                }
                decls.nominal.insert(name);
            }
            "type_item" => {
                let value = if node.child_by_field_name("type_parameters").is_some() {
                    None
                } else {
                    node.child_by_field_name("type")
                };
                let alias = decls.aliases.entry(name).or_insert(AliasDecl {
                    count: 0,
                    value: None,
                });
                alias.count += 1;
                alias.value = value;
            }
            _ => {
                decls.nominal.insert(name);
            }
        }
    }
    decls
}

fn resolve_aliases<'s>(
    cx: &TypeCx<'s>,
    decls: &TypeDecls<'s, '_>,
) -> std::collections::HashMap<&'s str, Option<String>> {
    decls
        .aliases
        .iter()
        .map(|(&name, alias)| {
            let target = alias
                .value
                .filter(|_| alias.count == 1 && !decls.nominal.contains(name))
                .and_then(|ty| extract_type_repr(cx, ty))
                .filter(|repr| !decls.aliases.contains_key(repr.as_str()));
            (name, target)
        })
        .collect()
}

/// Per-file type facts shared by receiver extraction; `None` entries mark non-authoritative names.
struct TypeCx<'s> {
    source: &'s [u8],
    lexical: LexicalNames,
    aliases: std::collections::HashMap<&'s str, Option<String>>,
    unit_structs: std::collections::HashSet<&'s str>,
    assoc_returns:
        std::collections::HashMap<&'s str, std::collections::HashMap<&'s str, Option<String>>>,
}

impl<'s> TypeCx<'s> {
    fn new(root: tree_sitter::Node<'_>, source: &'s [u8]) -> Self {
        let decls = collect_type_decls(root, source);
        let mut cx = Self {
            source,
            lexical: collect_lexical_names(root, source),
            aliases: std::collections::HashMap::new(),
            unit_structs: std::collections::HashSet::new(),
            assoc_returns: std::collections::HashMap::new(),
        };
        cx.aliases = resolve_aliases(&cx, &decls);
        cx.assoc_returns = collect_assoc_returns(&cx, root);
        cx.unit_structs = decls.unit;
        cx
    }
}

/// Bounds gathered for one type-parameter name; a name with several constraints has no unique bound.
#[derive(Default)]
struct Constraints<'s> {
    count: usize,
    bound: Option<&'s str>,
}

impl<'s> Constraints<'s> {
    fn add(&mut self, bound: Option<&'s str>) {
        self.count += 1;
        self.bound = if self.count == 1 { bound } else { None };
    }

    fn unique(self) -> Option<&'s str> {
        self.bound
    }
}

enum ParamLookup {
    Bound(String),
    Declared,
    Absent,
}

fn strip_generic_args(text: &str) -> &str {
    text.split(['<', '(']).next().unwrap_or(text).trim()
}

/// The single positive trait bound of a parameter; `None` when there are several or none.
fn extract_single_bound<'s>(bounds: tree_sitter::Node<'_>, source: &'s [u8]) -> Option<&'s str> {
    let mut trait_bound = None;
    let mut cursor = bounds.walk();
    for child in bounds.children(&mut cursor) {
        if matches!(child.kind(), "lifetime" | ":" | "+") {
            continue;
        }
        let target = if child.kind() == "higher_ranked_trait_bound" {
            child.child_by_field_name("type")?
        } else {
            child
        };
        let text = target.utf8_text(source).ok()?.trim();
        if text.starts_with('?') || text.is_empty() {
            continue;
        }
        if trait_bound.is_some() {
            return None;
        }
        trait_bound = Some(strip_generic_args(text));
    }
    trait_bound
}

fn where_clause_of(scope: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let mut cursor = scope.walk();
    scope
        .children(&mut cursor)
        .find(|child| child.kind() == "where_clause")
}

fn add_bounds<'s>(owner: tree_sitter::Node<'_>, source: &'s [u8], acc: &mut Constraints<'s>) {
    if let Some(bounds) = owner.child_by_field_name("bounds") {
        acc.add(extract_single_bound(bounds, source));
    }
}

fn declared_parameter_bounds<'s>(
    scope: tree_sitter::Node<'_>,
    name: &str,
    source: &'s [u8],
    acc: &mut Constraints<'s>,
) -> bool {
    let Some(params) = scope.child_by_field_name("type_parameters") else {
        return false;
    };
    let mut cursor = params.walk();
    let mut declared = false;
    for param in params.children(&mut cursor).filter(|param| {
        param.kind() == "type_parameter" && field_text(*param, "name", source) == Some(name)
    }) {
        declared = true;
        add_bounds(param, source, acc);
    }
    declared
}

/// Adds the bounds `scope` places on `name`; returns whether `scope` declares `name` as a type parameter.
fn scope_constraints<'s>(
    scope: tree_sitter::Node<'_>,
    name: &str,
    source: &'s [u8],
    acc: &mut Constraints<'s>,
) -> bool {
    let declared = declared_parameter_bounds(scope, name, source, acc);
    if let Some(clause) = where_clause_of(scope) {
        let mut cursor = clause.walk();
        for pred in clause.children(&mut cursor).filter(|pred| {
            pred.kind() == "where_predicate" && field_text(*pred, "left", source) == Some(name)
        }) {
            add_bounds(pred, source, acc);
        }
    }
    declared
}

fn resolve_type_param(cx: &TypeCx<'_>, node: tree_sitter::Node<'_>, name: &str) -> ParamLookup {
    let mut constraints = Constraints::default();
    let mut curr = Some(node);
    while let Some(n) = curr {
        if matches!(
            n.kind(),
            "function_item" | "function_signature_item" | "impl_item" | "trait_item"
        ) && scope_constraints(n, name, cx.source, &mut constraints)
        {
            return match constraints.unique() {
                Some(bound) => ParamLookup::Bound(bound.to_string()),
                None => ParamLookup::Declared,
            };
        }
        curr = n.parent();
    }
    ParamLookup::Absent
}

fn name_repr(cx: &TypeCx<'_>, at: tree_sitter::Node<'_>, name: &str) -> Option<String> {
    match resolve_type_param(cx, at, name) {
        ParamLookup::Bound(bound) => Some(bound),
        ParamLookup::Declared => Some(name.to_string()),
        ParamLookup::Absent => match cx.aliases.get(name) {
            Some(resolved) => resolved.clone(),
            None => Some(name.to_string()),
        },
    }
}

/// The nominal name of an impl subject or bound, with references and generic arguments removed but no wrapper unwrapping.
fn nominal_name<'s>(source: &'s [u8], ty: tree_sitter::Node<'_>) -> Option<&'s str> {
    match ty.kind() {
        "reference_type" | "generic_type" => nominal_name(source, ty.child_by_field_name("type")?),
        "type_identifier" | "scoped_type_identifier" => Some(ty.utf8_text(source).ok()?.trim()),
        _ => None,
    }
}

fn enclosing_container(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let mut curr = node.parent();
    while let Some(n) = curr {
        if matches!(n.kind(), "impl_item" | "trait_item") {
            return Some(n);
        }
        curr = n.parent();
    }
    None
}

fn enclosing_self_type(cx: &TypeCx<'_>, node: tree_sitter::Node<'_>) -> Option<String> {
    let container = enclosing_container(node)?;
    if container.kind() == "impl_item" {
        let subject = nominal_name(cx.source, container.child_by_field_name("type")?)?;
        return name_repr(cx, container, subject);
    }
    field_text(container, "name", cx.source).map(str::to_string)
}

fn self_or_name<'a>(
    cx: &TypeCx<'_>,
    at: tree_sitter::Node<'_>,
    name: &'a str,
) -> Option<std::borrow::Cow<'a, str>> {
    if name == "Self" {
        enclosing_self_type(cx, at).map(std::borrow::Cow::Owned)
    } else {
        Some(std::borrow::Cow::Borrowed(name))
    }
}

fn first_type_argument(args: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        if !matches!(child.kind(), "<" | ">" | "," | "lifetime") {
            return Some(child);
        }
    }
    None
}

/// Whether `path`, as written at `at`, denotes the std `Box`, `Arc`, or `Rc`.
/// Qualified paths name std directly; bare names resolve lexically, and only an unshadowed `Box` is the prelude.
fn names_std_wrapper(cx: &TypeCx<'_>, at: tree_sitter::Node<'_>, path: &str) -> bool {
    let path = path.trim_start_matches("::");
    if path.contains("::") {
        return WRAPPER_PATHS.contains(&path);
    }
    match cx.lexical.lookup(at, path) {
        Some(NameBinding::Import(target)) => WRAPPER_PATHS.contains(&target.as_str()),
        Some(NameBinding::Local) => false,
        None => path == "Box",
    }
}

fn generic_type_repr(cx: &TypeCx<'_>, ty: tree_sitter::Node<'_>) -> Option<String> {
    let outer = ty.child_by_field_name("type")?;
    let outer_text = outer.utf8_text(cx.source).ok()?.trim();
    if names_std_wrapper(cx, ty, outer_text)
        && let Some(inner) = ty
            .child_by_field_name("type_arguments")
            .and_then(first_type_argument)
    {
        return extract_type_repr(cx, inner);
    }
    if outer.kind() == "type_identifier" {
        name_repr(cx, ty, outer_text)
    } else {
        Some(outer_text.to_string())
    }
}

fn extract_type_repr(cx: &TypeCx<'_>, ty: tree_sitter::Node<'_>) -> Option<String> {
    match ty.kind() {
        "reference_type" => extract_type_repr(cx, ty.child_by_field_name("type")?),
        "dynamic_type" | "abstract_type" => extract_type_repr(cx, ty.child_by_field_name("trait")?),
        "generic_type" => generic_type_repr(cx, ty),
        "type_identifier" => {
            let text = ty.utf8_text(cx.source).ok()?.trim();
            if text == "Self" {
                enclosing_self_type(cx, ty)
            } else {
                name_repr(cx, ty, text)
            }
        }
        "scoped_type_identifier" => Some(ty.utf8_text(cx.source).ok()?.trim().to_string()),
        _ => None,
    }
}

/// Strips generic arguments from a path so `Type::<T>::f` and `Type::f` name the same subject.
fn bare_path(text: &str) -> &str {
    text.split('<')
        .next()
        .unwrap_or(text)
        .trim()
        .trim_end_matches("::")
}

fn first_call_argument(call: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let args = call.child_by_field_name("arguments")?;
    let mut cursor = args.walk();
    args.children(&mut cursor)
        .find(|arg| !matches!(arg.kind(), "(" | ")" | ","))
}

fn associated_call_type(cx: &TypeCx<'_>, subject: &str, name: &str) -> Option<String> {
    match cx.assoc_returns.get(subject).and_then(|fns| fns.get(name)) {
        Some(ret) => ret.clone(),
        None if name == "new" => Some(subject.to_string()),
        None => None,
    }
}

fn extract_call_type(
    cx: &TypeCx<'_>,
    func: tree_sitter::Node<'_>,
    call: tree_sitter::Node<'_>,
) -> Option<String> {
    if func.kind() != "scoped_identifier" {
        return None;
    }
    let path = bare_path(field_text(func, "path", cx.source)?);
    let name = field_text(func, "name", cx.source)?;
    if name == "new" && names_std_wrapper(cx, func, path) {
        return first_call_argument(call).and_then(|arg| extract_let_value_type(cx, arg));
    }
    let subject = self_or_name(cx, func, path)?;
    associated_call_type(cx, &subject, name)
}

fn extract_let_value_type(cx: &TypeCx<'_>, value: tree_sitter::Node<'_>) -> Option<String> {
    match value.kind() {
        "call_expression" => extract_call_type(cx, value.child_by_field_name("function")?, value),
        "struct_expression" => {
            let name = field_text(value, "name", cx.source)?;
            self_or_name(cx, value, name).map(std::borrow::Cow::into_owned)
        }
        "identifier" => {
            let name = value.utf8_text(cx.source).ok()?;
            cx.unit_structs.contains(name).then(|| name.to_string())
        }
        _ => None,
    }
}

fn extract_let_type(cx: &TypeCx<'_>, let_decl: tree_sitter::Node<'_>) -> Option<String> {
    match let_decl.child_by_field_name("type") {
        Some(ty) => extract_type_repr(cx, ty),
        None => extract_let_value_type(cx, let_decl.child_by_field_name("value")?),
    }
}

fn extract_param_type(cx: &TypeCx<'_>, param: tree_sitter::Node<'_>) -> Option<String> {
    extract_type_repr(cx, param.child_by_field_name("type")?)
}

fn self_binding_type(cx: &TypeCx<'_>, node: tree_sitter::Node<'_>) -> Option<String> {
    enclosing_param(node)
        .and_then(|param| extract_param_type(cx, param))
        .or_else(|| enclosing_self_type(cx, node))
}

/// A pattern whose only wrappers are `mut`: the bound identifier is the whole matched value.
fn is_direct_binding(node: tree_sitter::Node<'_>, pattern: tree_sitter::Node<'_>) -> bool {
    let mut cur = node;
    while cur != pattern {
        match cur.parent() {
            Some(parent) if parent.kind() == "mut_pattern" => cur = parent,
            _ => return false,
        }
    }
    true
}

fn binding_type(cx: &TypeCx<'_>, node: tree_sitter::Node<'_>) -> Option<String> {
    if let Some(let_decl) = enclosing_let(node) {
        let pattern = let_decl.child_by_field_name("pattern")?;
        return if is_direct_binding(node, pattern) {
            extract_let_type(cx, let_decl)
        } else {
            None
        };
    }
    let param = enclosing_param(node)?;
    let pattern = param.child_by_field_name("pattern")?;
    if is_direct_binding(node, pattern) {
        extract_param_type(cx, param)
    } else {
        None
    }
}

fn record_assoc<'s>(
    map: &mut std::collections::HashMap<
        &'s str,
        std::collections::HashMap<&'s str, Option<String>>,
    >,
    subject: &'s str,
    name: &'s str,
    ret: Option<String>,
) {
    let fns = map.entry(subject).or_default();
    match fns.entry(name) {
        std::collections::hash_map::Entry::Vacant(slot) => {
            slot.insert(ret);
        }
        std::collections::hash_map::Entry::Occupied(mut slot) => {
            if *slot.get() != ret {
                *slot.get_mut() = None;
            }
        }
    }
}

/// Associated function return types keyed by `(impl subject, fn name)`; conflicting declarations become `None`.
fn collect_assoc_returns<'s>(
    cx: &TypeCx<'s>,
    root: tree_sitter::Node<'_>,
) -> std::collections::HashMap<&'s str, std::collections::HashMap<&'s str, Option<String>>> {
    let mut out = std::collections::HashMap::new();
    for imp in super::reads::nodes(root).filter(|node| node.kind() == "impl_item") {
        let Some(subject) = imp
            .child_by_field_name("type")
            .and_then(|ty| nominal_name(cx.source, ty))
        else {
            continue;
        };
        let Some(body) = imp.child_by_field_name("body") else {
            continue;
        };
        let mut cursor = body.walk();
        for item in body
            .children(&mut cursor)
            .filter(|child| child.kind() == "function_item")
        {
            let Some(name) = field_text(item, "name", cx.source) else {
                continue;
            };
            let ret = item
                .child_by_field_name("return_type")
                .and_then(|ty| extract_type_repr(cx, ty));
            record_assoc(&mut out, subject, name, ret);
        }
    }
    out
}

fn enclosing_let(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let mut curr = node.parent();
    while let Some(n) = curr {
        if n.kind() == "let_declaration" {
            return Some(n);
        }
        if rust_scope(n.kind()) {
            break;
        }
        curr = n.parent();
    }
    None
}

fn enclosing_param(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let mut curr = node.parent();
    while let Some(n) = curr {
        if n.kind() == "parameter" {
            return Some(n);
        }
        if rust_scope(n.kind()) {
            break;
        }
        curr = n.parent();
    }
    None
}

fn is_self_binding(node: tree_sitter::Node<'_>) -> bool {
    node.kind() == "self_parameter"
        || (node.kind() == "self" && node.parent().is_some_and(|p| p.kind() == "parameter"))
}

fn collect_rust_read_bindings<'a>(
    root: tree_sitter::Node<'_>,
    source: &'a [u8],
) -> super::reads::ReadBindings<'a> {
    let cx = TypeCx::new(root, source);
    let mut bindings = super::reads::ReadBindings::default();
    for node in super::reads::nodes(root) {
        if is_self_binding(node) {
            if let Some(func) = super::reads::enclosing_scope(node, rust_scope) {
                bindings.insert("self", func.byte_range(), self_binding_type(&cx, node));
            }
        } else if let Some(range) = rust_binding_range(node)
            && let Ok(name) = node.utf8_text(source)
        {
            bindings.insert(name, range, binding_type(&cx, node));
        }
    }
    bindings
}

fn implementation_fact(
    node: tree_sitter::Node<'_>,
    source: &[u8],
) -> Option<super::Implementation> {
    let subject = nominal_name(source, node.child_by_field_name("type")?)?;
    let mut bounds = Constraints::default();
    let generic_subject = scope_constraints(node, subject, source, &mut bounds);
    Some(super::Implementation {
        trait_path: node
            .child_by_field_name("trait")
            .and_then(|ty| nominal_name(source, ty))
            .map(str::to_string),
        subject: subject.to_string(),
        subject_bound: if generic_subject {
            bounds.unique().map(str::to_string)
        } else {
            None
        },
        generic_subject,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    })
}

fn collect_rust_implementations(
    root: tree_sitter::Node<'_>,
    source: &[u8],
) -> Vec<super::Implementation> {
    super::reads::nodes(root)
        .filter(|node| node.kind() == "impl_item")
        .filter_map(|node| implementation_fact(node, source))
        .collect()
}
