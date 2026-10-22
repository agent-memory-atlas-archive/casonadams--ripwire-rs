use super::ast::{ancestor, contains_kind, text, visit};
use super::syntax::call_positions;
use crate::metrics::lint_rules::{FileLintContext, Sink, one_line};
use tree_sitter::Node;

const NODE_CONTAINERS: &[&str] = &[
    "map",
    "multimap",
    "set",
    "multiset",
    "list",
    "forward_list",
    "unordered_map",
    "unordered_multimap",
    "unordered_set",
    "unordered_multiset",
];
const LOOPS: &[&str] = &[
    "for_statement",
    "for_range_loop",
    "while_statement",
    "do_statement",
];

fn template_name<'a>(ctx: &'a FileLintContext<'_>, node: Node<'_>) -> &'a str {
    node.child_by_field_name("name")
        .or_else(|| node.named_child(0))
        .map_or("", |n| text(ctx, n))
}

pub fn node_container(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "template_type" && NODE_CONTAINERS.contains(&template_name(ctx, node)) {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

fn vector_arg(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("arguments")?.named_child(0)
}

pub fn vector_of_raw_ptr(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "template_type"
            && template_name(ctx, node) == "vector"
            && vector_arg(node).is_some_and(|a| text(ctx, a).trim_end().ends_with('*'))
        {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

pub fn vector_of_indirect(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() != "template_type" || template_name(ctx, node) != "vector" {
            return;
        }
        if vector_arg(node).is_some_and(|a| {
            let raw = text(ctx, a);
            ["unique_ptr", "shared_ptr", "vector"]
                .iter()
                .any(|n| raw.contains(n))
        }) {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

fn loop_body(node: Node<'_>) -> bool {
    let mut current = node;
    while let Some(p) = current.parent() {
        if LOOPS.contains(&p.kind()) {
            return p.child_by_field_name("body").is_some_and(|b| b == current);
        }
        current = p;
    }
    false
}

pub fn heap_alloc_in_loop(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    if !ctx.sem.supports_cache_lint() {
        return;
    }
    visit(ctx, |node| {
        let alloc = node.kind() == "new_expression"
            || (node.kind() == "call_expression"
                && node.child_by_field_name("function").is_some_and(|n| {
                    ["malloc", "calloc", "realloc", "strdup"].contains(&text(ctx, n))
                }));
        if alloc && loop_body(node) {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

pub fn pointer_chase_loop(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    if !ctx.sem.supports_cache_lint() {
        return;
    }
    visit(ctx, |node| {
        if node.kind() != "assignment_expression" || !loop_body(node) {
            return;
        }
        let Some(left) = node.child_by_field_name("left") else {
            return;
        };
        let Some(right) = node.child_by_field_name("right") else {
            return;
        };
        let lhs = text(ctx, left);
        if text(ctx, right)
            .strip_prefix(lhs)
            .is_some_and(|tail| tail.trim_start().starts_with("->"))
        {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

pub fn gather_subscript(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    if !ctx.sem.supports_cache_lint() {
        return;
    }
    visit(ctx, |node| {
        if node.kind() == "subscript_expression"
            && contains_kind(node, "subscript_expression")
            && !ancestor(node, &["subscript_expression"])
            && loop_body(node)
        {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

pub fn shared_ptr_by_value(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "parameter_declaration" {
            let raw = text(ctx, node);
            if raw.contains("shared_ptr") && !raw.contains(['&', '*']) {
                sink.at(node.start_byte(), one_line(raw));
            }
        }
    });
}

pub fn manual_prefetch(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    for name in ["_mm_prefetch", "__builtin_prefetch"] {
        for at in call_positions(&ctx.code, name) {
            sink.at(at, name);
        }
    }
}
