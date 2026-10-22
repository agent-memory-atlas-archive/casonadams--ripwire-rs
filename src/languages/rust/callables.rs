use super::super::CallableBinding;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use tree_sitter::Node;

#[derive(Default)]
struct Syntax<'tree> {
    aliases: Vec<Node<'tree>>,
    fields: Vec<Node<'tree>>,
    macros: Vec<Node<'tree>>,
}

impl<'tree> Syntax<'tree> {
    fn collect(root: Node<'tree>) -> Self {
        let mut syntax = Self::default();
        for node in super::super::reads::nodes(root) {
            let target = match node.kind() {
                "type_item" => &mut syntax.aliases,
                "field_declaration" => &mut syntax.fields,
                "macro_definition" => &mut syntax.macros,
                _ => continue,
            };
            target.push(node);
        }
        syntax
    }
}

fn text<'source>(node: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    node.utf8_text(source).ok()
}

fn field_text<'source>(node: Node<'_>, field: &str, source: &'source [u8]) -> Option<&'source str> {
    text(node.child_by_field_name(field)?, source)
}

fn unique_named<'tree, 'source>(
    definitions: &[Node<'tree>],
    source: &'source [u8],
) -> HashMap<&'source str, Node<'tree>> {
    let mut unique = HashMap::new();
    let mut duplicates = HashSet::new();
    for &node in definitions {
        let Some(name) = field_text(node, "name", source) else {
            continue;
        };
        if unique.insert(name, node).is_some() {
            duplicates.insert(name);
        }
    }
    unique.retain(|name, _| !duplicates.contains(name));
    unique
}

fn callable_fields<'source>(
    syntax: &Syntax<'_>,
    source: &'source [u8],
) -> (
    HashMap<&'source str, HashSet<&'source str>>,
    Vec<CallableBinding>,
) {
    let aliases: HashSet<_> = unique_named(&syntax.aliases, source)
        .into_iter()
        .filter(|(_, node)| {
            node.child_by_field_name("type")
                .is_some_and(|ty| ty.kind() == "function_type")
        })
        .map(|(name, _)| name)
        .collect();
    let mut fields: HashMap<_, HashSet<_>> = HashMap::new();
    let mut declarations = Vec::new();
    for &field in &syntax.fields {
        let Some(ty) = field.child_by_field_name("type") else {
            continue;
        };
        if ty.kind() != "function_type"
            && !text(ty, source).is_some_and(|name| aliases.contains(name))
        {
            continue;
        }
        let Some(owner) = field.parent().and_then(|body| body.parent()) else {
            continue;
        };
        if owner.kind() != "struct_item" {
            continue;
        }
        let Some(name) = field_text(owner, "name", source) else {
            continue;
        };
        if let Some(field_name) = field_text(field, "name", source) {
            fields.entry(name).or_default().insert(field_name);
            declarations.push(CallableBinding {
                owner: name.to_owned(),
                field: field_name.to_owned(),
                target: None,
                start_byte: field.start_byte(),
            });
        }
    }
    (fields, declarations)
}

fn identifier(value: &str) -> bool {
    let value = value.strip_prefix("r#").unwrap_or(value);
    let mut chars = value.chars();
    chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
        && chars.all(|c| c == '_' || c.is_alphanumeric())
}

fn path(value: &str) -> Option<String> {
    let mut result = String::with_capacity(value.len());
    for segment in value.split("::").map(str::trim) {
        if !identifier(segment) {
            return None;
        }
        if !result.is_empty() {
            result.push_str("::");
        }
        result.push_str(segment);
    }
    Some(result)
}

fn direct_binding(
    node: Node<'_>,
    fields: &HashMap<&str, HashSet<&str>>,
    source: &[u8],
    out: &mut Vec<CallableBinding>,
) {
    let Some(owner) = field_text(node, "name", source) else {
        return;
    };
    let Some(allowed) = fields.get(owner) else {
        return;
    };
    let Some(body) = node.child_by_field_name("body") else {
        return;
    };
    let mut cursor = body.walk();
    out.extend(body.named_children(&mut cursor).filter_map(|field| {
        let name = field_text(field, "field", source)?;
        if !allowed.contains(name) {
            return None;
        }
        let value = field.child_by_field_name("value")?;
        if !matches!(value.kind(), "identifier" | "scoped_identifier") {
            return None;
        }
        Some(CallableBinding {
            owner: owner.to_owned(),
            field: name.to_owned(),
            target: Some(path(text(value, source)?)?),
            start_byte: value.start_byte(),
        })
    }));
}

fn segments(node: Node<'_>) -> impl Iterator<Item = Range<usize>> {
    let mut start = 1;
    std::iter::from_fn(move || {
        while start + 1 < node.child_count() {
            let begin = start;
            while start + 1 < node.child_count() && node.child(start)?.kind() != "," {
                start += 1;
            }
            let end = start;
            start += 1;
            if begin != end {
                return Some(begin..end);
            }
        }
        None
    })
}

fn tokens(node: Node<'_>, range: Range<usize>) -> impl Iterator<Item = Node<'_>> {
    range
        .filter_map(move |index| node.child(index))
        .filter(|child| !matches!(child.kind(), "line_comment" | "block_comment"))
}

struct Projection<'source> {
    owner: &'source str,
    field: &'source str,
    argument: usize,
}

struct Macro<'source> {
    arity: usize,
    projections: Vec<Projection<'source>>,
}

fn parameters<'source>(
    pattern: Node<'_>,
    source: &'source [u8],
) -> Option<Vec<(&'source str, &'source str)>> {
    segments(pattern)
        .map(|range| {
            let mut children = tokens(pattern, range);
            let binding = children.next()?;
            if binding.kind() != "token_binding_pattern" || children.next().is_some() {
                return None;
            }
            Some((
                field_text(binding, "name", source)?,
                field_text(binding, "type", source)?,
            ))
        })
        .collect()
}

fn projection<'source>(
    tree: Node<'_>,
    range: Range<usize>,
    source: &'source [u8],
) -> Option<(&'source str, &'source str)> {
    let mut children = tokens(tree, range);
    let field = children.next()?;
    if children.next()?.kind() != ":" {
        return None;
    }
    let parameter = children.next()?;
    if parameter.kind() != "metavariable" || children.next().is_some() {
        return None;
    }
    Some((text(field, source)?, text(parameter, source)?))
}

fn macro_projections<'source>(
    template: Node<'_>,
    fields: &HashMap<&str, HashSet<&str>>,
    parameters: &[(&str, &str)],
    source: &'source [u8],
) -> Vec<Projection<'source>> {
    let mut projections = Vec::new();
    for tree in super::super::reads::nodes(template).filter(|node| node.kind() == "token_tree") {
        let Some(owner) = tree
            .prev_named_sibling()
            .and_then(|node| text(node, source))
        else {
            continue;
        };
        let Some(allowed) = fields.get(owner) else {
            continue;
        };
        projections.extend(segments(tree).filter_map(|range| {
            let (field, parameter) = projection(tree, range, source)?;
            if !allowed.contains(field) {
                return None;
            }
            let argument = parameters
                .iter()
                .position(|&(name, kind)| name == parameter && kind == "path")?;
            Some(Projection {
                owner,
                field,
                argument,
            })
        }));
    }
    projections
}

fn macro_model<'source>(
    node: Node<'_>,
    fields: &HashMap<&str, HashSet<&str>>,
    source: &'source [u8],
) -> Option<Macro<'source>> {
    let mut cursor = node.walk();
    let mut rules = node
        .named_children(&mut cursor)
        .filter(|node| node.kind() == "macro_rule");
    let rule = rules.next()?;
    if rules.next().is_some() {
        return None;
    }
    let parameters = parameters(rule.child_by_field_name("left")?, source)?;
    let template = rule.child_by_field_name("right")?;
    let projections = macro_projections(template, fields, &parameters, source);
    if projections.is_empty() {
        return None;
    }
    Some(Macro {
        arity: parameters.len(),
        projections,
    })
}

fn macro_models<'source>(
    syntax: &Syntax<'_>,
    fields: &HashMap<&str, HashSet<&str>>,
    source: &'source [u8],
) -> HashMap<&'source str, Macro<'source>> {
    unique_named(&syntax.macros, source)
        .into_iter()
        .filter_map(|(name, node)| macro_model(node, fields, source).map(|model| (name, model)))
        .collect()
}

fn macro_bindings(
    node: Node<'_>,
    models: &HashMap<&str, Macro<'_>>,
    source: &[u8],
    out: &mut Vec<CallableBinding>,
) {
    let Some(model) = field_text(node, "macro", source).and_then(|name| models.get(name)) else {
        return;
    };
    let mut cursor = node.walk();
    let Some(arguments) = node
        .named_children(&mut cursor)
        .find(|node| node.kind() == "token_tree")
    else {
        return;
    };
    let arguments: Vec<_> = segments(arguments)
        .filter_map(|range| {
            let mut children = tokens(arguments, range);
            let first = children.next()?;
            let end = children.last().unwrap_or(first).end_byte();
            source
                .get(first.start_byte()..end)
                .and_then(|bytes| std::str::from_utf8(bytes).ok())
        })
        .collect();
    if arguments.len() != model.arity {
        return;
    }
    out.extend(model.projections.iter().filter_map(|projection| {
        Some(CallableBinding {
            owner: projection.owner.to_owned(),
            field: projection.field.to_owned(),
            target: Some(path(arguments[projection.argument])?),
            start_byte: node.start_byte(),
        })
    }));
}

pub(super) fn collect(root: Node<'_>, source: &[u8]) -> Vec<CallableBinding> {
    let syntax = Syntax::collect(root);
    let (fields, mut bindings) = callable_fields(&syntax, source);
    if fields.is_empty() {
        return Vec::new();
    }
    let models = macro_models(&syntax, &fields, source);
    for node in super::super::reads::nodes(root) {
        match node.kind() {
            "struct_expression" => direct_binding(node, &fields, source, &mut bindings),
            "macro_invocation" => macro_bindings(node, &models, source, &mut bindings),
            _ => {}
        }
    }
    bindings.sort_unstable_by(|a, b| {
        (&a.owner, &a.field, &a.target, a.start_byte).cmp(&(
            &b.owner,
            &b.field,
            &b.target,
            b.start_byte,
        ))
    });
    bindings.dedup_by(|a, b| {
        a.owner == b.owner
            && a.field == b.field
            && a.target == b.target
            && a.start_byte == b.start_byte
    });
    bindings
}
