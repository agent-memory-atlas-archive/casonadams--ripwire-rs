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

fn is_param_node(parent: Node<'_>) -> bool {
    let pkind = parent.kind();
    pkind.contains("parameter")
        || pkind.contains("param")
        || pkind == "formal_parameters"
        || pkind == "parameters"
}

fn is_decl_kind(pkind: &str) -> bool {
    pkind.contains("declarator")
        || pkind.contains("declaration")
        || pkind == "let_binding"
        || pkind == "initialized_identifier"
        || pkind == "initialized_variable_definition"
        || pkind == "static_final_declaration"
}

fn is_decl_node(parent: Node<'_>, name_node: Node<'_>) -> bool {
    if is_decl_kind(parent.kind()) {
        if let Some(left) = parent.child_by_field_name("name") {
            return left == name_node;
        }
        if let Some(first) = parent.named_child(0) {
            return first == name_node;
        }
        return true;
    }
    false
}

fn is_assign_node(parent: Node<'_>, name_node: Node<'_>) -> bool {
    let pkind = parent.kind();
    if pkind.contains("assign") || pkind == "assignment_expression" {
        if let Some(left) = parent.child_by_field_name("left") {
            return left == name_node;
        }
        if let Some(first) = parent.child(0) {
            return first == name_node;
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
    let Some(parent) = node.parent() else {
        return OccRole::Read;
    };

    if is_param_node(parent) {
        OccRole::Param
    } else if is_decl_node(parent, node) {
        OccRole::Decl
    } else if is_assign_node(parent, node) {
        OccRole::Assign
    } else if is_call_arg_node(node) {
        OccRole::CallArg
    } else {
        OccRole::Read
    }
}

fn is_identifier_node(kind: &str) -> bool {
    kind == "identifier"
        || kind == "type_identifier"
        || kind == "field_identifier"
        || kind == "simple_identifier"
        || kind == "name"
}

fn collect_occurrences_recursive(
    node: Node<'_>,
    source_bytes: &[u8],
    target_var: Option<&str>,
    out: &mut Vec<VariableOcc>,
) {
    let kind = node.kind();
    if is_identifier_node(kind)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_occ_role_properties() {
        let roles = [
            (OccRole::Param, true, false, "param"),
            (OccRole::Decl, true, false, "decl"),
            (OccRole::Assign, true, true, "assign"),
            (OccRole::CallArg, false, true, "call-arg"),
            (OccRole::Read, false, true, "read"),
        ];
        for (r, def, is_use, tag) in roles {
            assert_eq!(r.is_def(), def);
            assert_eq!(r.is_use(), is_use);
            assert_eq!(r.as_str(), tag);
        }
    }
}
