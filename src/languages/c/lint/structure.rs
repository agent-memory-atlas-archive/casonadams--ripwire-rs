use crate::languages::common::lint::ast::{contains_kind, text, visit};
use crate::metrics::lint_rules::{FileLintContext, Sink, one_line};

pub fn suspicious_semicolon(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "if_statement"
            && node
                .child_by_field_name("consequence")
                .is_some_and(|body| text(ctx, body).trim() == ";")
        {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

pub fn empty_catch(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() != "catch_clause" {
            return;
        }
        if let Some(body) = node.child_by_field_name("body") {
            let raw = text(ctx, body)
                .trim()
                .trim_start_matches('{')
                .trim_end_matches('}');
            if raw.trim().is_empty() {
                sink.at(node.start_byte(), one_line(text(ctx, node)));
            }
        }
    });
}

pub fn deep_nesting(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    for &s in ctx.syms.iter().filter(|s| s.sym.is_callable()) {
        let mut depth = 0u32;
        let mut deepest = (0, 0);
        for (offset, b) in ctx.body(s.sym).bytes().enumerate() {
            if b == b'{' {
                depth += 1;
                if depth > deepest.0 {
                    deepest = (depth, offset);
                }
            } else if b == b'}' {
                depth = depth.saturating_sub(1);
            }
        }
        if deepest.0 > 4 {
            sink.at(
                s.sym.sig_end_byte as usize + deepest.1,
                format!("brace nesting depth {}", deepest.0),
            );
        }
    }
}

pub fn inconsistent_return(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    for &s in ctx.syms.iter().filter(|s| s.sym.is_callable()) {
        let body = ctx.body(s.sym);
        if let Some(bare) = body.find("return;")
            && body.contains("return ")
        {
            sink.at(
                s.sym.sig_end_byte as usize + bare,
                "bare return mixed with value return",
            );
        }
    }
}

pub fn typedef_over_using(ctx: &FileLintContext<'_>, sink: &mut Sink<'_>) {
    visit(ctx, |node| {
        if node.kind() == "type_definition" && !contains_kind(node, "function_declarator") {
            sink.at(node.start_byte(), one_line(text(ctx, node)));
        }
    });
}

