//! Declared-type facts TypeScript exposes to call resolution: class `implements` records, typed
//! binding types (parameters, annotated or `new`-initialized declarators), and object-literal member
//! types for member-chain receivers such as `input.queue` where `input: { queue: TaskEnqueuer }`.

use crate::languages::Implementation;
use crate::languages::javascript::javascript_binding_scope;
use crate::languages::reads::{ReadBindings, nodes};
use tree_sitter::Node;

/// Every class (with its own method span) and every `implements` name it declares.
pub(super) fn class_implementations(root: Node<'_>, source: &[u8]) -> Vec<Implementation> {
    nodes(root)
        .filter(|node| matches!(node.kind(), "class_declaration" | "abstract_class_declaration"))
        .filter_map(|class| {
            let subject = class.child_by_field_name("name")?.utf8_text(source).ok()?;
            let span = (class.start_byte(), class.end_byte());
            let own = implementation(subject, None, span);
            let implemented = implemented_names(class, source)
                .into_iter()
                .map(|name| implementation(subject, Some(name), span));
            Some(std::iter::once(own).chain(implemented).collect::<Vec<_>>())
        })
        .flatten()
        .collect()
}

fn implementation(subject: &str, trait_path: Option<&str>, span: (usize, usize)) -> Implementation {
    Implementation {
        subject: subject.to_string(),
        trait_path: trait_path.map(str::to_string),
        subject_bound: None,
        generic_subject: false,
        start_byte: span.0,
        end_byte: span.1,
    }
}

/// Names in a class's `implements` clause: plain and generic interface references.
fn implemented_names<'s>(class: Node<'_>, source: &'s [u8]) -> Vec<&'s str> {
    let mut cursor = class.walk();
    class
        .children(&mut cursor)
        .filter(|heritage| heritage.kind() == "class_heritage")
        .flat_map(|heritage| {
            let mut inner = heritage.walk();
            heritage
                .children(&mut inner)
                .filter(|clause| clause.kind() == "implements_clause")
                .flat_map(|clause| {
                    let mut types = clause.walk();
                    clause
                        .named_children(&mut types)
                        .filter_map(|ty| match ty.kind() {
                            "type_identifier" => ty.utf8_text(source).ok(),
                            "generic_type" => ty
                                .child_by_field_name("name")
                                .and_then(|name| name.utf8_text(source).ok()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Read bindings with declared types: typed parameters, annotated declarators, and `const x = new C()`.
pub(super) fn typed_read_bindings<'a>(root: Node<'_>, source: &'a [u8]) -> ReadBindings<'a> {
    let mut bindings = ReadBindings::collect(root, source, |node| {
        javascript_binding_scope(node).map(|scope| scope.byte_range())
    });
    for node in nodes(root) {
        if node.kind() == "function_declaration" {
            insert_return_type(&mut bindings, node, source);
            continue;
        }
        let (name, annotation, value) = match node.kind() {
            "required_parameter" | "optional_parameter" => (
                node.child_by_field_name("pattern"),
                node.child_by_field_name("type"),
                None,
            ),
            "variable_declarator" => (
                node.child_by_field_name("name"),
                node.child_by_field_name("type"),
                node.child_by_field_name("value"),
            ),
            _ => continue,
        };
        let Some(name) = name.filter(|name| name.kind() == "identifier") else {
            continue;
        };
        let ty = annotation
            .and_then(|ann| ann.utf8_text(source).ok())
            .map(|text| text.trim_start_matches(':').trim().to_string())
            .or_else(|| value.and_then(|value| constructed_type(value, source)));
        let (Some(ty), Ok(text), Some(scope)) = (
            ty,
            name.utf8_text(source),
            javascript_binding_scope(name).map(|scope| scope.byte_range()),
        ) else {
            continue;
        };
        bindings.insert(text, scope, Some(ty));
    }
    bindings
}

/// A module-level function with an explicitly annotated return type: a call `f()` in this file has
/// that declared result type. Nested functions and unannotated functions record nothing.
fn insert_return_type<'a>(bindings: &mut ReadBindings<'a>, function: Node<'_>, source: &'a [u8]) {
    let module_level = function
        .parent()
        .is_some_and(|parent| matches!(parent.kind(), "program" | "export_statement"));
    let (true, Some(name), Some(ret)) = (
        module_level,
        function.child_by_field_name("name"),
        function.child_by_field_name("return_type"),
    ) else {
        return;
    };
    if let (Ok(text), Ok(ret_text)) = (name.utf8_text(source), ret.utf8_text(source)) {
        let ty = ret_text.trim_start_matches(':').trim();
        bindings.insert_return(text, ty.to_string());
    }
}

/// The class named by `new C(...)`, which is the declared type of the value it initializes.
fn constructed_type(value: Node<'_>, source: &[u8]) -> Option<String> {
    (value.kind() == "new_expression")
        .then(|| value.child_by_field_name("constructor"))
        .flatten()
        .filter(|ctor| ctor.kind() == "identifier")
        .and_then(|ctor| ctor.utf8_text(source).ok())
        .map(str::to_string)
}

/// Declared type of member `member` of an object-literal type `{ member: T; … }`, as written.
pub(super) fn object_member_type(receiver_type: &str, member: &str) -> Option<String> {
    let body = receiver_type
        .trim()
        .strip_prefix('{')?
        .strip_suffix('}')?;
    top_level_members(body)
        .into_iter()
        .find_map(|part| {
            let (key, value) = part.split_once(':')?;
            (key.trim().trim_end_matches('?').trim() == member).then(|| value.trim().to_string())
        })
}

/// Splits an object type body at `;` and `,` outside nested brackets.
fn top_level_members(body: &str) -> Vec<&str> {
    let mut depth = 0i32;
    let mut parts = Vec::new();
    let mut start = 0;
    for (idx, byte) in body.bytes().enumerate() {
        match byte {
            b'{' | b'(' | b'[' | b'<' => depth += 1,
            b'}' | b')' | b']' | b'>' => depth -= 1,
            b';' | b',' if depth == 0 => {
                parts.push(&body[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[start..]);
    parts
}
