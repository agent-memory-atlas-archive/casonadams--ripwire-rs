use std::collections::HashMap;
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingSpan {
    pub range: std::ops::Range<usize>,
    pub ty: Option<String>,
}

#[derive(Default)]
pub struct ReadBindings<'a> {
    scopes: HashMap<&'a str, Vec<BindingSpan>>,
}

impl<'a> ReadBindings<'a> {
    pub fn collect(
        root: Node<'_>,
        source: &'a [u8],
        binding_scope: impl Fn(Node<'_>) -> Option<std::ops::Range<usize>>,
    ) -> Self {
        let mut result = Self::default();
        let mut cursor = root.walk();
        loop {
            let node = cursor.node();
            if let Some(scope) = binding_scope(node)
                && let Ok(name) = node.utf8_text(source)
            {
                result.insert(name, scope, None);
            }
            if !advance(&mut cursor) {
                return result;
            }
        }
    }

    pub fn insert(&mut self, name: &'a str, range: std::ops::Range<usize>, ty: Option<String>) {
        let spans = self.scopes.entry(name).or_default();
        if let Some(existing) = spans.iter_mut().find(|s| s.range == range) {
            if existing.ty.is_none() {
                existing.ty = ty;
            }
        } else {
            spans.push(BindingSpan { range, ty });
        }
    }

    pub fn shadows(&self, name: &str, byte: usize) -> bool {
        self.scopes
            .get(name)
            .is_some_and(|spans| spans.iter().any(|span| span.range.contains(&byte)))
    }

    pub fn type_at(&self, name: &str, byte: usize) -> Option<&str> {
        let spans = self.scopes.get(name)?;
        spans
            .iter()
            .filter(|span| span.range.contains(&byte))
            .max_by_key(|span| span.range.start)
            .and_then(|span| span.ty.as_deref())
    }
}

pub(super) fn advance(cursor: &mut tree_sitter::TreeCursor<'_>) -> bool {
    if cursor.goto_first_child() {
        return true;
    }
    while !cursor.goto_next_sibling() {
        if !cursor.goto_parent() {
            return false;
        }
    }
    true
}

pub(super) fn nodes(root: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let mut cursor = root.walk();
    let mut done = false;
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        let node = cursor.node();
        done = !advance(&mut cursor);
        Some(node)
    })
}

pub(super) fn in_field(parent: Node<'_>, node: Node<'_>, field: &str) -> bool {
    parent.child_by_field_name(field).is_some_and(|child| {
        child.start_byte() <= node.start_byte() && child.end_byte() >= node.end_byte()
    })
}

pub(super) fn enclosing_scope<'a>(
    node: Node<'a>,
    is_scope: impl Fn(&str) -> bool,
) -> Option<Node<'a>> {
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        if is_scope(parent.kind()) {
            return Some(parent);
        }
        ancestor = parent.parent();
    }
    None
}

pub(super) fn value_position(
    node: Node<'_>,
    excluded: impl Fn(Node<'_>, Node<'_>) -> bool,
    is_scope: impl Fn(&str) -> bool,
) -> bool {
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        if excluded(parent, node) {
            return false;
        }
        if is_scope(parent.kind()) {
            break;
        }
        ancestor = parent.parent();
    }
    true
}
