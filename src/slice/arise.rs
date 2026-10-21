use crate::languages::SyntaxKinds;
use serde::{Deserialize, Serialize};
use tree_sitter::Node;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OccRole {
    Param = 0,
    Decl,
    Assign,
    CallArg,
    Read,
}

impl OccRole {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        if self == Self::Param {
            "param"
        } else if self == Self::Decl {
            "decl"
        } else if self == Self::Assign {
            "assign"
        } else if self == Self::CallArg {
            "call-arg"
        } else {
            "read"
        }
    }
    #[must_use]
    pub const fn is_def(self) -> bool {
        matches!(self, Self::Param | Self::Decl | Self::Assign)
    }

    #[must_use]
    pub const fn is_use(self) -> bool {
        matches!(self, Self::CallArg | Self::Read | Self::Assign)
    }
}

impl std::fmt::Display for OccRole {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single intra-procedural variable occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableOcc {
    pub line: u32,
    pub stmt_line: u32,
    pub start_byte: usize,
    pub end_byte: usize,
    pub role: OccRole,
    pub var_name: String,
}

fn unwrap_binding_ancestor<'a>(node: Node<'a>, kinds: &SyntaxKinds) -> Node<'a> {
    let mut curr = node;
    while let Some(parent) = curr.parent() {
        if !kinds.binding_patterns.contains(&parent.kind())
            || parent.child_by_field_name("right") == Some(curr)
        {
            break;
        }
        curr = parent;
    }
    curr
}

fn is_rhs_or_type_child(parent: Node<'_>, child: Node<'_>) -> bool {
    ["value", "right", "type", "default_value", "arguments"]
        .into_iter()
        .any(|f| parent.child_by_field_name(f) == Some(child))
}

fn is_param_node(parent: Node<'_>, binding: Node<'_>) -> bool {
    if is_rhs_or_type_child(parent, binding) {
        return false;
    }
    let pkind = parent.kind();
    (pkind.contains("parameter")
        || pkind.contains("param")
        || pkind == "formal_parameters"
        || pkind == "parameters")
        && !pkind.contains("type_param")
        && !pkind.contains("template_param")
}

fn is_decl_kind(pkind: &str, kinds: &SyntaxKinds) -> bool {
    (pkind.contains("declarator")
        || pkind.contains("declaration")
        || kinds.local_bindings.contains(&pkind))
        && !kinds.non_local_declarations.contains(&pkind)
}

fn lhs_decl_field(parent: Node<'_>) -> Option<Node<'_>> {
    parent
        .child_by_field_name("name")
        .or_else(|| parent.child_by_field_name("pattern"))
        .or_else(|| parent.child_by_field_name("declarator"))
        .or_else(|| parent.child_by_field_name("left"))
}

/// True for a declarator naming the function of its enclosing definition (C `int f(int)`).
fn is_function_declarator(node: Node<'_>, kinds: &SyntaxKinds) -> bool {
    kinds.function_declarators.contains(&node.kind())
        && node
            .parent()
            .is_some_and(|gp| kinds.function_defs.contains(&gp.kind()))
}

fn is_decl_node(parent: Node<'_>, binding: Node<'_>, kinds: &SyntaxKinds) -> bool {
    if !is_decl_kind(parent.kind(), kinds)
        || is_rhs_or_type_child(parent, binding)
        || is_function_declarator(parent, kinds)
    {
        return false;
    }
    if let Some(lhs) = lhs_decl_field(parent) {
        return lhs == binding;
    }
    parent.named_child(0) == Some(binding)
}

fn is_assign_node(parent: Node<'_>, binding: Node<'_>) -> bool {
    let pkind = parent.kind();
    if pkind.contains("assign") || pkind == "update_expression" {
        if let Some(left) = parent
            .child_by_field_name("left")
            .or_else(|| parent.child_by_field_name("argument"))
        {
            return left == binding;
        }
        if let Some(first) = parent.named_child(0) {
            return first == binding;
        }
    }
    false
}

fn has_ancestor_before(
    node: Node<'_>,
    match_pred: impl Fn(&str) -> bool,
    stop_pred: impl Fn(&str) -> bool,
) -> bool {
    let mut curr = node.parent();
    while let Some(p) = curr {
        let pk = p.kind();
        if match_pred(pk) {
            return true;
        }
        if stop_pred(pk) {
            break;
        }
        curr = p.parent();
    }
    false
}

fn is_call_arg_node(node: Node<'_>) -> bool {
    has_ancestor_before(
        node,
        |pk| pk.contains("argument") || pk == "args",
        |pk| pk.contains("statement") || pk.contains("declaration"),
    )
}

fn find_stmt_line(node: Node<'_>) -> u32 {
    let mut curr = Some(node);
    while let Some(n) = curr {
        let nkind = n.kind();
        if nkind.contains("statement")
            || nkind.contains("declaration")
            || nkind == "expression_statement"
        {
            return n.start_position().row as u32 + 1;
        }
        curr = n.parent();
    }
    node.start_position().row as u32 + 1
}

fn classify_node_role(node: Node<'_>, kinds: &SyntaxKinds) -> OccRole {
    let binding = unwrap_binding_ancestor(node, kinds);
    let Some(parent) = binding.parent() else {
        return OccRole::Read;
    };

    if is_param_node(parent, binding) {
        OccRole::Param
    } else if is_decl_node(parent, binding, kinds) {
        OccRole::Decl
    } else if is_assign_node(parent, binding) {
        OccRole::Assign
    } else if is_call_arg_node(node) {
        OccRole::CallArg
    } else {
        OccRole::Read
    }
}

fn is_inside_type_context(node: Node<'_>, kinds: &SyntaxKinds) -> bool {
    has_ancestor_before(
        node,
        |pk| kinds.type_contexts.contains(&pk),
        |pk| pk.contains("statement") || pk.contains("body") || pk == "block",
    )
}

fn is_member_or_callee_ident(node: Node<'_>, kinds: &SyntaxKinds) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    let pk = parent.kind();
    if parent.child_by_field_name("function") == Some(node) {
        return true;
    }
    if kinds.function_defs.contains(&pk) && parent.child_by_field_name("name") == Some(node) {
        return true;
    }
    if is_function_declarator(parent, kinds)
        && parent.child_by_field_name("declarator") == Some(node)
    {
        return true;
    }
    if kinds.field_access.contains(&pk) || kinds.scoped_paths.contains(&pk) {
        let recv = parent
            .child_by_field_name("object")
            .or_else(|| parent.child_by_field_name("argument"))
            .or_else(|| parent.child_by_field_name("value"))
            .or_else(|| parent.child_by_field_name("path"))
            .or_else(|| parent.child_by_field_name("scope"));
        return recv != Some(node);
    }
    false
}

fn collect_occurrences_recursive(
    node: Node<'_>,
    source_bytes: &[u8],
    target_var: Option<&str>,
    kinds: &SyntaxKinds,
    out: &mut Vec<VariableOcc>,
) {
    if kinds.identifiers.contains(&node.kind())
        && !is_member_or_callee_ident(node, kinds)
        && !is_inside_type_context(node, kinds)
        && let Ok(name) = node.utf8_text(source_bytes)
        && (target_var.is_none() || target_var == Some(name))
    {
        out.push(VariableOcc {
            line: node.start_position().row as u32 + 1,
            stmt_line: find_stmt_line(node),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            role: classify_node_role(node, kinds),
            var_name: name.to_string(),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_occurrences_recursive(child, source_bytes, target_var, kinds, out);
    }
}

/// Collect all variable occurrences in the given function definition node.
#[must_use]
pub fn collect_symbol_variables(
    def_node: Node<'_>,
    source_bytes: &[u8],
    target_var: Option<&str>,
    kinds: &SyntaxKinds,
) -> Vec<VariableOcc> {
    let mut out = Vec::new();
    collect_occurrences_recursive(def_node, source_bytes, target_var, kinds, &mut out);
    out.sort_by_key(|o| o.start_byte);
    out
}

/// Extract unique defined local variable tuples `(var_name, line, role)` in order of first appearance.
#[must_use]
pub fn collect_unique_def_locals(occs: Vec<VariableOcc>) -> Vec<(String, u32, OccRole)> {
    let mut seen = std::collections::HashSet::new();
    occs.into_iter()
        .filter(|occ| occ.role.is_def() && seen.insert(occ.var_name.clone()))
        .map(|occ| (occ.var_name, occ.line, occ.role))
        .collect()
}

/// List unique sliceable local variable names in order of their first appearance.
#[must_use]
pub fn list_sliceable_locals(
    def_node: Node<'_>,
    source_bytes: &[u8],
    kinds: &SyntaxKinds,
) -> Vec<(String, u32, OccRole)> {
    collect_unique_def_locals(collect_symbol_variables(
        def_node,
        source_bytes,
        None,
        kinds,
    ))
}
