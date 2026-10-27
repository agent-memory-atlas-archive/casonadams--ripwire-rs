//! Java and C# grammars label types and names with fields, so bindings read them by field name.

use super::reads::{ReadBindings, enclosing_scope, nodes};
use crate::languages::{Implementation, is_plain_type_name};
use std::collections::HashMap;
use tree_sitter::Node;

pub struct TypedSyntax {
    pub classes: &'static [&'static str],
    pub supertype_clauses: &'static [&'static str],
    pub params: &'static [&'static str],
    pub functions: &'static [&'static str],
    pub locals: &'static [&'static str],
    pub fields: &'static [&'static str],
    pub declarators: &'static [&'static str],
    pub blocks: &'static [&'static str],
    pub constructions: &'static [&'static str],
    pub inferred: &'static [&'static str],
    pub assignments: &'static [&'static str],
    pub calls: &'static [(&'static str, &'static str)],
    pub return_type_field: &'static str,
    pub superclass: Option<(&'static str, &'static str)>,
    /// Scala: `val x: T` is a declaration whose own declarator is named by `pattern`.
    pub declaration_name: Option<&'static str>,
    pub class_params: &'static [&'static str],
}

pub fn class_implementations(
    root: Node<'_>,
    source: &[u8],
    syntax: &TypedSyntax,
) -> Vec<Implementation> {
    let mut out = Vec::new();
    for class in nodes(root).filter(|n| syntax.classes.contains(&n.kind())) {
        let Some(subject) = text_of(class.child_by_field_name("name"), source)
            .filter(|name| is_plain_type_name(name))
        else {
            continue;
        };
        let span = (class.start_byte(), class.end_byte());
        out.push(implementation(subject, None, span));
        let mut cursor = class.walk();
        for clause in class
            .named_children(&mut cursor)
            .filter(|n| syntax.supertype_clauses.contains(&n.kind()))
        {
            out.extend(
                type_names(clause, source).map(|name| implementation(subject, Some(name), span)),
            );
        }
    }
    out
}

fn type_names<'a>(clause: Node<'_>, source: &'a [u8]) -> impl Iterator<Item = &'a str> {
    nodes(clause)
        .filter(|n| matches!(n.kind(), "type_identifier" | "identifier" | "name"))
        .filter_map(move |n| n.utf8_text(source).ok())
        .collect::<Vec<_>>()
        .into_iter()
}

#[must_use]
pub fn implementation(
    subject: &str,
    trait_path: Option<&str>,
    span: (usize, usize),
) -> Implementation {
    Implementation {
        subject: subject.to_string(),
        trait_path: trait_path.map(str::to_string),
        subject_bound: None,
        generic_subject: false,
        start_byte: span.0,
        end_byte: span.1,
    }
}

pub fn typed_read_bindings<'a>(
    root: Node<'_>,
    source: &'a [u8],
    syntax: &TypedSyntax,
) -> ReadBindings<'a> {
    let mut bindings = ReadBindings::default();
    let mut returns = AgreedReturns::default();
    for node in nodes(root) {
        let kind = node.kind();
        if syntax.classes.contains(&kind) {
            record_class(&mut bindings, node, source, syntax);
        } else if syntax.params.contains(&kind) {
            record_parameter(&mut bindings, node, source, syntax);
        } else if syntax.locals.contains(&kind) {
            record_local(&mut bindings, node, source, syntax);
        } else if syntax.assignments.contains(&kind) {
            record_assignment(&mut bindings, node, source, syntax);
        } else if syntax.functions.contains(&kind) {
            record_return(&mut returns, node, source, syntax);
        }
    }
    returns.insert_into(&mut bindings);
    bindings
}

#[derive(Default)]
pub struct AgreedReturns<'a>(HashMap<&'a str, Option<String>>);

impl<'a> AgreedReturns<'a> {
    pub fn note(&mut self, name: &'a str, ty: Option<String>) {
        self.0
            .entry(name)
            .and_modify(|seen| {
                if *seen != ty {
                    *seen = None;
                }
            })
            .or_insert(ty);
    }

    pub fn insert_into(self, bindings: &mut ReadBindings<'a>) {
        for (name, ty) in self.0 {
            if let Some(ty) = ty {
                bindings.insert_return(name, ty);
            }
        }
    }
}

fn record_return<'a>(
    returns: &mut AgreedReturns<'a>,
    function: Node<'_>,
    source: &'a [u8],
    syntax: &TypedSyntax,
) {
    let Some(name) = text_of(function.child_by_field_name("name"), source) else {
        return;
    };
    let ty = text_of(
        function.child_by_field_name(syntax.return_type_field),
        source,
    )
    .filter(|ty| is_plain_type_name(ty))
    .map(str::to_string);
    returns.note(name, ty);
}

pub fn call_receiver_name<'a>(
    receiver: Node<'_>,
    source: &'a [u8],
    syntax: &TypedSyntax,
) -> Option<&'a str> {
    let &(_, callee_field) = syntax
        .calls
        .iter()
        .find(|(kind, _)| *kind == receiver.kind())?;
    if receiver.child_by_field_name("object").is_some() {
        return None;
    }
    let callee = receiver.child_by_field_name(callee_field)?;
    text_of(Some(callee), source).filter(|name| is_plain_type_name(name))
}

fn record_assignment<'a>(
    bindings: &mut ReadBindings<'a>,
    assignment: Node<'_>,
    source: &'a [u8],
    syntax: &TypedSyntax,
) {
    let (Some(left), Some(right)) = (
        assignment.child_by_field_name("left"),
        assignment.child_by_field_name("right"),
    ) else {
        return;
    };
    let (Some(name), Some(block)) = (
        text_of(Some(left), source).filter(|name| is_plain_type_name(name)),
        enclosing_scope(assignment, |kind| syntax.blocks.contains(&kind)),
    ) else {
        return;
    };
    let ty = syntax
        .constructions
        .contains(&right.kind())
        .then(|| construction_type(right, source))
        .flatten();
    bindings.insert(name, assignment.start_byte()..block.end_byte(), ty);
}

fn record_class<'a>(
    bindings: &mut ReadBindings<'a>,
    class: Node<'_>,
    source: &'a [u8],
    syntax: &TypedSyntax,
) {
    let (Some(name), Some(body)) = (
        text_of(class.child_by_field_name("name"), source),
        class.child_by_field_name("body"),
    ) else {
        return;
    };
    bindings.insert("this", body.byte_range(), Some(name.to_string()));
    if let Some((clause_kind, keyword)) = syntax.superclass {
        let mut cursor = class.walk();
        let base = class
            .named_children(&mut cursor)
            .find(|n| n.kind() == clause_kind)
            .and_then(|clause| type_names(clause, source).next());
        if let Some(base) = base {
            bindings.insert(keyword, body.byte_range(), Some(base.to_string()));
        }
    }
    let mut cursor = body.walk();
    let body_fields: Vec<Node<'_>> = body
        .named_children(&mut cursor)
        .filter(|n| syntax.fields.contains(&n.kind()))
        .collect();
    let mut fields: Vec<(&str, Option<String>)> = body_fields
        .into_iter()
        .flat_map(|field| {
            let declared = declared_type(field, source, syntax);
            declarator_types(field, declared.as_deref(), source, syntax)
        })
        .collect();
    fields.extend(
        children_and_grandchildren(class)
            .filter(|n| syntax.class_params.contains(&n.kind()))
            .filter_map(|param| {
                let name = binding_name(param, source, syntax)?;
                Some((name, declared_type(param, source, syntax)))
            }),
    );
    for (field_name, ty) in fields {
        if let Some(ty) = &ty {
            bindings.insert_member(name, field_name, ty.clone());
        }
        bindings.insert(field_name, body.byte_range(), ty);
    }
}

fn binding_name<'a>(node: Node<'_>, source: &'a [u8], syntax: &TypedSyntax) -> Option<&'a str> {
    if let Some(name) = node.child_by_field_name("name") {
        return text_of(Some(name), source);
    }
    let mut current = node.child_by_field_name(syntax.declaration_name?)?;
    loop {
        match current.kind() {
            "identifier" | "field_identifier" => return current.utf8_text(source).ok(),
            "function_declarator" => return None,
            kind if kind.ends_with("declarator") => {
                current = current
                    .child_by_field_name("declarator")
                    .or_else(|| current.named_child(0))?;
            }
            _ => return None,
        }
    }
}

fn record_parameter<'a>(
    bindings: &mut ReadBindings<'a>,
    param: Node<'_>,
    source: &'a [u8],
    syntax: &TypedSyntax,
) {
    let (Some(name), Some(function)) = (
        binding_name(param, source, syntax),
        enclosing_scope(param, |kind| syntax.functions.contains(&kind)),
    ) else {
        return;
    };
    bindings.insert(
        name,
        function.byte_range(),
        declared_type(param, source, syntax),
    );
}

fn record_local<'a>(
    bindings: &mut ReadBindings<'a>,
    local: Node<'_>,
    source: &'a [u8],
    syntax: &TypedSyntax,
) {
    let Some(block) = enclosing_scope(local, |kind| syntax.blocks.contains(&kind)) else {
        return;
    };
    let declared = declared_type(local, source, syntax);
    for (name, ty) in declarator_types(local, declared.as_deref(), source, syntax) {
        bindings.insert(name, local.start_byte()..block.end_byte(), ty);
    }
}

fn declarator_types<'a>(
    declaration: Node<'_>,
    declared: Option<&str>,
    source: &'a [u8],
    syntax: &TypedSyntax,
) -> Vec<(&'a str, Option<String>)> {
    if syntax.declaration_name.is_some() {
        let ty = declared
            .map(str::to_string)
            .or_else(|| constructed_type(declaration, source, syntax));
        return binding_name(declaration, source, syntax)
            .map(|name| vec![(name, ty)])
            .unwrap_or_default();
    }
    let mut out = Vec::new();
    for declarator in
        children_and_grandchildren(declaration).filter(|n| syntax.declarators.contains(&n.kind()))
    {
        let Some(name) = text_of(declarator.child_by_field_name("name"), source) else {
            continue;
        };
        let ty = declared
            .map(str::to_string)
            .or_else(|| constructed_type(declarator, source, syntax));
        out.push((name, ty));
    }
    out
}

/// C# fields wrap their type in a `variable_declaration` child.
fn declared_type(node: Node<'_>, source: &[u8], syntax: &TypedSyntax) -> Option<String> {
    let ty = node.child_by_field_name("type").or_else(|| {
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .find_map(|child| child.child_by_field_name("type"))
    })?;
    let text = ty.utf8_text(source).ok()?;
    (is_plain_type_name(text) && !syntax.inferred.contains(&text)).then(|| text.to_string())
}

fn constructed_type(declarator: Node<'_>, source: &[u8], syntax: &TypedSyntax) -> Option<String> {
    let mut cursor = declarator.walk();
    let construction = declarator
        .named_children(&mut cursor)
        .find(|n| syntax.constructions.contains(&n.kind()))?;
    construction_type(construction, source)
}

/// PHP `new Connector()` names the class in a bare `name` node.
fn construction_type(construction: Node<'_>, source: &[u8]) -> Option<String> {
    let ty = construction
        .child_by_field_name("type")
        .or_else(|| construction.named_child(0));
    let text = text_of(ty, source)?;
    is_plain_type_name(text).then(|| text.to_string())
}

fn children_and_grandchildren(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let mut cursor = node.walk();
    let children: Vec<Node<'_>> = node.named_children(&mut cursor).collect();
    let grandchildren: Vec<Node<'_>> = children
        .iter()
        .flat_map(|child| {
            let mut inner = child.walk();
            child.named_children(&mut inner).collect::<Vec<_>>()
        })
        .collect();
    children.into_iter().chain(grandchildren)
}

/// PHP variable nodes include the `$` sigil.
fn text_of<'a>(node: Option<Node<'_>>, source: &'a [u8]) -> Option<&'a str> {
    Some(node?.utf8_text(source).ok()?.trim_start_matches('$'))
}
