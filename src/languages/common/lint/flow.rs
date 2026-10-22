use super::ast::{text, visit};
use crate::metrics::lint_rules::{FileLintContext, Sink, one_line};
use tree_sitter::Node;

pub fn goto(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "goto_statement" {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

pub fn do_while(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "do_statement" {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

fn is_jump(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "return_statement"
            | "break_statement"
            | "continue_statement"
            | "throw_statement"
            | "raise_statement"
    )
}

fn is_comment(node: Node<'_>) -> bool {
    node.kind().contains("comment")
}

pub fn unreachable_code(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if !is_jump(node)
            || !node.parent().is_some_and(|p| {
                matches!(p.kind(), "block" | "compound_statement" | "statement_block")
            })
        {
            return;
        }
        let mut next = node.next_named_sibling();
        while next.is_some_and(is_comment) {
            next = next.and_then(|n| n.next_named_sibling());
        }
        if let Some(statement) = next {
            sink.at(statement.start_byte(), one_line(text(ctx, statement)));
        }
    });
}
