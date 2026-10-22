use super::ast::{text, visit};
use super::syntax::{call_end, call_positions};
use crate::metrics::lint_rules::{FileLintContext, Sink, one_line};

pub fn c_style_cast(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "cast_expression" {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

fn calls(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>, names: &[&str]) {
    for &name in names {
        for at in call_positions(&ctx.code, name) {
            let end = call_end(&ctx.code, at, name);
            sink.at(at, one_line(&ctx.raw[at..end]));
        }
    }
}

pub fn unsafe_c_fn(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    calls(ctx, sink, &["strcpy", "strcat", "sprintf", "gets"]);
}

pub fn weak_crypto(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    calls(ctx, sink, &["MD5", "SHA1", "MD4", "RC4"]);
}

pub fn redundant_parens(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "parenthesized_expression"
            && node
                .parent()
                .is_some_and(|p| p.kind() == "parenthesized_expression")
        {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

pub fn self_assign(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if !matches!(
            node.kind(),
            "assignment_expression"
                | "augmented_assignment_expression"
                | "assignment_expression_without_cascade"
        ) {
            return;
        }
        let Some(lhs) = node
            .child_by_field_name("left")
            .or_else(|| node.named_child(0))
        else {
            return;
        };
        let Some(rhs) = node
            .child_by_field_name("right")
            .or_else(|| node.named_child(1))
        else {
            return;
        };
        if ctx.code[lhs.end_byte()..rhs.start_byte()].trim() != "=" {
            return;
        }
        let left = text(ctx, lhs);
        let right = ctx.sem.assigned_value_root(text(ctx, rhs));
        if left == right {
            sink.at(node.start_byte(), format!("{left} = {right}"));
        }
    });
}
