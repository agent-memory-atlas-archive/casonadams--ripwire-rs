use crate::metrics::lint_rules::FileLintContext;
use tree_sitter::Node;

pub fn visit(ctx: &FileLintContext<'_>, mut check: impl FnMut(Node<'_>)) {
    let Some(tree) = &ctx.tree else { return };
    let mut cursor = tree.walk();
    loop {
        check(cursor.node());
        if cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

pub fn text<'a>(ctx: &'a FileLintContext<'_>, node: Node<'_>) -> &'a str {
    ctx.raw.get(node.byte_range()).unwrap_or("")
}

pub fn ancestor(node: Node<'_>, kinds: &[&str]) -> bool {
    let mut parent = node.parent();
    while let Some(p) = parent {
        if kinds.contains(&p.kind()) {
            return true;
        }
        parent = p.parent();
    }
    false
}

pub fn contains_kind(node: Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| child.kind() == kind || contains_kind(child, kind))
}
