use super::ast::{ancestor, contains_kind, text, visit};
use super::syntax::decimal_value;
use crate::metrics::lint_rules::{FileLintContext, Sink, one_line};
use std::collections::BTreeSet;
use tree_sitter::Node;

fn in_for_header(node: Node<'_>) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.kind() == "for_statement" {
            return parent
                .child_by_field_name("body")
                .is_some_and(|b| current != b);
        }
        current = parent;
    }
    false
}

pub fn magic_number(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    let mut seen = BTreeSet::new();
    visit(ctx, |node| {
        if node.kind() != "number_literal"
            || !ctx.in_callable(node.start_byte())
            || ctx.sem.lint_constant_initializer(node, ctx.raw)
        {
            return;
        }
        let number = node
            .parent()
            .filter(|p| p.kind() == "unary_expression" && text(ctx, *p).starts_with('-'))
            .unwrap_or(node);
        let raw = text(ctx, number);
        if decimal_value(raw).is_some_and(|n| ![-1.0, 0.0, 1.0, 2.0].contains(&n))
            && seen.insert((ctx.line_of(node.start_byte()), raw))
        {
            sink.at(number.start_byte(), raw);
        }
    });
}

pub fn comma_operator(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "comma_expression"
            && !in_for_header(node)
            && !node
                .parent()
                .is_some_and(|p| p.kind() == "comma_expression")
        {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

pub fn embedded_crement(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "update_expression"
            && !in_for_header(node)
            && !node
                .parent()
                .is_some_and(|p| p.kind() == "expression_statement")
        {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

fn assignment_value(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    !matches!(
        parent.kind(),
        "expression_statement" | "init_declarator" | "declaration"
    ) && !in_for_header(node)
}

pub fn assign_as_value(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "assignment_expression" && assignment_value(node) {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

pub fn nested_ternary(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "conditional_expression"
            && contains_kind(node, "conditional_expression")
            && !ancestor(node, &["conditional_expression"])
        {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

fn arithmetic_condition(ctx: &FileLintContext<'_>, node: Node<'_>) -> bool {
    if node.kind() == "number_literal" {
        return decimal_value(text(ctx, node)).is_some_and(|n| n != 0.0 && n != 1.0);
    }
    node.kind() == "binary_expression"
        && node
            .child_by_field_name("operator")
            .is_some_and(|op| matches!(text(ctx, op), "+" | "-" | "*" | "/" | "%"))
}

pub fn implicit_predicate(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if !matches!(node.kind(), "if_statement" | "while_statement") {
            return;
        }
        let Some(mut condition) = node.child_by_field_name("condition") else {
            return;
        };
        while matches!(
            condition.kind(),
            "parenthesized_expression" | "condition_clause"
        ) {
            let Some(inner) = condition.named_child(0) else {
                return;
            };
            condition = inner;
        }
        if arithmetic_condition(ctx, condition) {
            sink.at(condition.start_byte(), one_line(text(ctx, condition)));
        }
    });
}

pub fn octal_literal(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        let raw = text(ctx, node);
        if node.kind() == "number_literal"
            && raw.len() > 1
            && raw.starts_with('0')
            && raw.bytes().all(|b| (b'0'..=b'7').contains(&b))
        {
            sink.at(node.start_byte(), raw);
        }
    });
}

pub fn reversed_subscript(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "subscript_expression"
            && node
                .named_child(0)
                .is_some_and(|n| n.kind() == "number_literal")
        {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}
