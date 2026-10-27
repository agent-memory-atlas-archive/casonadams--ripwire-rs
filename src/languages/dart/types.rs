use crate::languages::reads::{ReadBindings, nodes};
use crate::languages::typed::{AgreedReturns, implementation};
use crate::languages::{Implementation, is_plain_type_name};
use tree_sitter::Node;

pub(super) fn class_implementations(root: Node<'_>, source: &[u8]) -> Vec<Implementation> {
    nodes(root)
        .filter(|node| node.kind() == "class_definition")
        .filter_map(|class| {
            let subject = class.child_by_field_name("name")?.utf8_text(source).ok()?;
            let span = (class.start_byte(), class.end_byte());
            let mut out = vec![implementation(subject, None, span)];
            for clause in ["superclass", "interfaces"]
                .into_iter()
                .filter_map(|field| class.child_by_field_name(field))
            {
                let mut cursor = clause.walk();
                out.extend(
                    clause
                        .named_children(&mut cursor)
                        .filter(|ty| ty.kind() == "type_identifier")
                        .filter_map(|ty| ty.utf8_text(source).ok())
                        .map(|name| implementation(subject, Some(name), span)),
                );
            }
            Some(out)
        })
        .flatten()
        .collect()
}

pub(super) fn typed_read_bindings<'a>(root: Node<'_>, source: &'a [u8]) -> ReadBindings<'a> {
    let mut bindings = ReadBindings::default();
    let mut returns = AgreedReturns::default();
    for node in nodes(root) {
        match node.kind() {
            "class_definition" => record_class(&mut bindings, node, source),
            "formal_parameter" => record_parameter(&mut bindings, node, source),
            "initialized_variable_definition" => record_local(&mut bindings, node, source),
            "function_signature" => record_return(&mut returns, node, source),
            _ => {}
        }
    }
    returns.insert_into(&mut bindings);
    bindings
}

fn record_return<'a>(returns: &mut AgreedReturns<'a>, signature: Node<'_>, source: &'a [u8]) {
    let Some(name) = signature
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
    else {
        return;
    };
    let mut cursor = signature.walk();
    let generic = signature
        .children(&mut cursor)
        .any(|child| child.kind() == "type_arguments");
    let ty = (!generic)
        .then(|| declared_type(signature, source))
        .flatten();
    returns.note(name, ty);
}

fn record_class<'a>(bindings: &mut ReadBindings<'a>, class: Node<'_>, source: &'a [u8]) {
    let (Some(name), Some(body)) = (
        class
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source).ok()),
        class.child_by_field_name("body"),
    ) else {
        return;
    };
    bindings.insert("this", body.byte_range(), Some(name.to_string()));
    if let Some(base) = class
        .child_by_field_name("superclass")
        .and_then(|clause| declared_type(clause, source))
    {
        bindings.insert("super", body.byte_range(), Some(base));
    }
    let mut cursor = body.walk();
    for declaration in body
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "declaration")
    {
        let Some(ty) = declared_type(declaration, source) else {
            continue;
        };
        for field in field_names(declaration, source) {
            bindings.insert(field, body.byte_range(), Some(ty.clone()));
            bindings.insert_member(name, field, ty.clone());
        }
    }
}

fn field_names<'a>(declaration: Node<'_>, source: &'a [u8]) -> Vec<&'a str> {
    let mut cursor = declaration.walk();
    declaration
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "initialized_identifier_list")
        .flat_map(|list| {
            let mut inner = list.walk();
            list.named_children(&mut inner)
                .filter_map(|item| item.named_child(0))
                .filter(|ident| ident.kind() == "identifier")
                .filter_map(|ident| ident.utf8_text(source).ok())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn record_parameter<'a>(bindings: &mut ReadBindings<'a>, param: Node<'_>, source: &'a [u8]) {
    let Some(name) = param
        .child_by_field_name("name")
        .filter(|n| n.kind() == "identifier")
        .and_then(|n| n.utf8_text(source).ok())
    else {
        return;
    };
    if let Some(scope) = parameter_scope(param) {
        bindings.insert(name, scope, declared_type(param, source));
    }
}

fn parameter_scope(param: Node<'_>) -> Option<std::ops::Range<usize>> {
    let mut owner = param.parent()?;
    while matches!(
        owner.kind(),
        "formal_parameter_list" | "optional_formal_parameters"
    ) {
        owner = owner.parent()?;
    }
    if owner.kind() == "function_expression" {
        return Some(owner.byte_range());
    }
    let holder = match owner.parent() {
        Some(parent) if parent.kind() == "method_signature" => parent,
        _ => owner,
    };
    let body = super::dart_following_body(holder)?;
    Some(holder.start_byte()..body.end_byte())
}

fn record_local<'a>(bindings: &mut ReadBindings<'a>, definition: Node<'_>, source: &'a [u8]) {
    let Some(name) = definition
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
    else {
        return;
    };
    let Some(block) = crate::languages::reads::enclosing_scope(definition, |kind| kind == "block")
    else {
        return;
    };
    let ty = declared_type(definition, source).or_else(|| constructed_type(definition, source));
    bindings.insert(name, definition.start_byte()..block.end_byte(), ty);
}

fn declared_type(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    let ty = node
        .children(&mut cursor)
        .find(|child| child.kind() == "type_identifier")?
        .utf8_text(source)
        .ok()?;
    is_plain_type_name(ty).then(|| ty.to_string())
}

fn constructed_type(definition: Node<'_>, source: &[u8]) -> Option<String> {
    let value = definition.child_by_field_name("value")?;
    let call = value.next_named_sibling()?;
    let name = value.utf8_text(source).ok()?;
    (value.kind() == "identifier"
        && call.kind() == "selector"
        && call.utf8_text(source).ok()?.starts_with('(')
        && name.starts_with(char::is_uppercase)
        && is_plain_type_name(name))
    .then(|| name.to_string())
}
