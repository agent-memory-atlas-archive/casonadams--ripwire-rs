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

fn is_binding_wrapper_kind(kind: &str) -> bool {
    matches!(
        kind,
        "tuple_pattern"
            | "slice_pattern"
            | "tuple_struct_pattern"
            | "struct_pattern"
            | "field_pattern"
            | "mut_pattern"
            | "ref_pattern"
            | "pointer_declarator"
            | "reference_declarator"
            | "array_declarator"
            | "parenthesized_declarator"
            | "object_pattern"
            | "array_pattern"
            | "pair_pattern"
            | "rest_pattern"
            | "assignment_pattern"
    )
}

fn unwrap_binding_ancestor(node: Node<'_>) -> Node<'_> {
    let mut curr = node;
    while let Some(parent) = curr.parent() {
        if parent.kind() == "assignment_pattern"
            && parent.child_by_field_name("right") == Some(curr)
        {
            break;
        }
        if !is_binding_wrapper_kind(parent.kind()) {
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

fn is_decl_kind(pkind: &str) -> bool {
    (pkind.contains("declarator")
        || pkind.contains("declaration")
        || pkind == "let_binding"
        || pkind == "let_condition"
        || pkind == "for_in_statement"
        || pkind == "for_range_loop"
        || pkind == "range_clause"
        || pkind == "initialized_identifier"
        || pkind == "initialized_variable_definition"
        || pkind == "static_final_declaration")
        && !matches!(
            pkind,
            "function_declaration"
                | "class_declaration"
                | "interface_declaration"
                | "type_alias_declaration"
                | "enum_declaration"
        )
}

fn lhs_decl_field(parent: Node<'_>) -> Option<Node<'_>> {
    parent
        .child_by_field_name("name")
        .or_else(|| parent.child_by_field_name("pattern"))
        .or_else(|| parent.child_by_field_name("declarator"))
        .or_else(|| parent.child_by_field_name("left"))
}

fn is_decl_node(parent: Node<'_>, binding: Node<'_>) -> bool {
    if !is_decl_kind(parent.kind()) || is_rhs_or_type_child(parent, binding) {
        return false;
    }
    if parent.kind() == "function_declarator"
        && parent
            .parent()
            .is_some_and(|gp| gp.kind() == "function_definition")
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
    if pkind.contains("assign") || pkind == "assignment_expression" || pkind == "update_expression"
    {
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

fn is_call_arg_node(node: Node<'_>) -> bool {
    let mut curr = node.parent();
    while let Some(parent) = curr {
        let pkind = parent.kind();
        if pkind.contains("argument") || pkind == "args" {
            return true;
        }
        if pkind.contains("statement") || pkind.contains("declaration") {
            break;
        }
        curr = parent.parent();
    }
    false
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

fn classify_node_role(node: Node<'_>) -> OccRole {
    let binding = unwrap_binding_ancestor(node);
    let Some(parent) = binding.parent() else {
        return OccRole::Read;
    };

    if is_param_node(parent, binding) {
        OccRole::Param
    } else if is_decl_node(parent, binding) {
        OccRole::Decl
    } else if is_assign_node(parent, binding) {
        OccRole::Assign
    } else if is_call_arg_node(node) {
        OccRole::CallArg
    } else {
        OccRole::Read
    }
}

fn is_identifier_node(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "simple_identifier"
            | "shorthand_property_identifier_pattern"
            | "variable_name"
            | "name"
    )
}

fn is_inside_type_context(node: Node<'_>) -> bool {
    let mut curr = node.parent();
    while let Some(p) = curr {
        let pk = p.kind();
        if matches!(
            pk,
            "type_parameters"
                | "type_parameter"
                | "type_annotation"
                | "type_arguments"
                | "generic_type"
                | "template_argument_list"
                | "template_parameter_list"
        ) {
            return true;
        }
        if pk.contains("statement") || pk.contains("body") || pk == "block" {
            break;
        }
        curr = p.parent();
    }
    false
}

fn is_member_or_callee_ident(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    let pk = parent.kind();
    if parent.child_by_field_name("function") == Some(node) {
        return true;
    }
    if matches!(
        pk,
        "function_declaration" | "function_definition" | "function_item" | "method_definition"
    ) && parent.child_by_field_name("name") == Some(node)
    {
        return true;
    }
    if pk == "function_declarator"
        && parent.child_by_field_name("declarator") == Some(node)
        && parent
            .parent()
            .is_some_and(|gp| gp.kind() == "function_definition")
    {
        return true;
    }
    if matches!(
        pk,
        "member_expression"
            | "field_expression"
            | "attribute"
            | "scoped_identifier"
            | "qualified_identifier"
    ) {
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
    out: &mut Vec<VariableOcc>,
) {
    let kind = node.kind();
    if is_identifier_node(kind)
        && !is_member_or_callee_ident(node)
        && !is_inside_type_context(node)
        && let Ok(name) = node.utf8_text(source_bytes)
        && (target_var.is_none() || target_var == Some(name))
    {
        let role = classify_node_role(node);
        let line = node.start_position().row as u32 + 1;
        let stmt_line = find_stmt_line(node);
        out.push(VariableOcc {
            line,
            stmt_line,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            role,
            var_name: name.to_string(),
        });
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_occurrences_recursive(child, source_bytes, target_var, out);
        }
    }
}

/// Collect all variable occurrences in the given function definition node.
#[must_use]
pub fn collect_symbol_variables(
    def_node: Node<'_>,
    source_bytes: &[u8],
    target_var: Option<&str>,
) -> Vec<VariableOcc> {
    let mut out = Vec::new();
    collect_occurrences_recursive(def_node, source_bytes, target_var, &mut out);
    out.sort_by_key(|o| o.start_byte);
    out
}

/// List unique sliceable local variable names in order of their first appearance.
#[must_use]
pub fn list_sliceable_locals(
    def_node: Node<'_>,
    source_bytes: &[u8],
) -> Vec<(String, u32, OccRole)> {
    let occs = collect_symbol_variables(def_node, source_bytes, None);
    let mut seen = std::collections::HashSet::new();
    let mut locals = Vec::new();

    for occ in occs {
        if occ.role.is_def() && seen.insert(occ.var_name.clone()) {
            locals.push((occ.var_name, occ.line, occ.role));
        }
    }
    locals
}
