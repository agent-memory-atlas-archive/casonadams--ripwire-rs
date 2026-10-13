use crate::domain::Lang;
use serde::{Deserialize, Serialize};
use tree_sitter::Node;
/// Nesting threshold for counting humps and deep lines.
pub const NESTING_BAR: u16 = 4;

/// Computed complexity and nesting metrics for a symbol or code block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ComplexityMetrics {
    pub cx: u32,
    pub ccx: u32,
    pub max_nest: u16,
    pub humps: u16,
    pub deep: u16,
    pub ev: u16,
}

#[must_use]
pub fn is_cyclomatic_decision(kind: &str) -> bool {
    matches!(
        kind,
        "if_statement"
            | "if_expression"
            | "for_statement"
            | "for_range_loop"
            | "for_in_statement"
            | "for_expression"
            | "while_statement"
            | "while_expression"
            | "do_statement"
            | "loop_expression"
            | "do_while_statement"
            | "repeat_statement"
            | "elseif_statement"
            | "match_conditional_expression"
            | "case_statement"
            | "match_arm"
            | "expression_case"
            | "communication_case"
            | "catch_clause"
            | "except_clause"
            | "catch_block"
            | "conditional_expression"
            | "ternary_expression"
            | "boolean_operator"
            | "when_entry"
            | "pattern_section"
            | "switch_section"
            | "switch_expression_arm"
            | "guard_statement"
            | "if"
            | "elsif"
            | "unless"
            | "while"
            | "until"
            | "for"
            | "rescue"
            | "when"
    )
}

#[must_use]
pub fn is_nesting_control(kind: &str) -> bool {
    matches!(
        kind,
        "if_statement"
            | "if_expression"
            | "for_statement"
            | "for_range_loop"
            | "for_in_statement"
            | "for_expression"
            | "while_statement"
            | "while_expression"
            | "do_statement"
            | "loop_expression"
            | "do_while_statement"
            | "repeat_statement"
            | "switch_statement"
            | "switch_expression"
            | "match_expression"
            | "catch_clause"
            | "except_clause"
            | "catch_block"
            | "when_expression"
            | "match_statement"
            | "conditional_expression"
            | "ternary_expression"
            | "if"
            | "unless"
    )
}

#[must_use]
pub fn is_jump_node(kind: &str) -> bool {
    matches!(
        kind,
        "break_statement"
            | "continue_statement"
            | "goto_statement"
            | "return_statement"
            | "throw_statement"
            | "break"
            | "continue"
            | "return"
    )
}

#[must_use]
pub fn is_bool_operator(kind: &str, text: &str) -> bool {
    kind == "boolean_operator"
        || kind == "binary_expression"
            && (text == "&&" || text == "||" || text == "and" || text == "or")
        || text == "&&"
        || text == "||"
}

struct HumpTracker {
    humps: u16,
    deep_lines: u16,
    deep_end_line: u32,
}

impl HumpTracker {
    fn new() -> Self {
        Self {
            humps: 0,
            deep_lines: 0,
            deep_end_line: 0,
        }
    }

    fn note_nesting(&mut self, from_nest: u16, to_nest: u16, node: &Node<'_>) {
        if from_nest >= NESTING_BAR || to_nest < NESTING_BAR {
            return;
        }
        self.humps = self.humps.saturating_add(1);
        let start_line = node.start_position().row as u32 + 1;
        let end_line = node.end_position().row as u32 + 1;
        let from = if start_line > self.deep_end_line {
            start_line
        } else {
            self.deep_end_line + 1
        };
        if end_line >= from {
            let lines = (end_line - from + 1) as u16;
            self.deep_lines = self.deep_lines.saturating_add(lines);
        }
        if end_line > self.deep_end_line {
            self.deep_end_line = end_line;
        }
    }
}

struct ComplexityState {
    cx: u32,
    ccx: u32,
    max_nest: u16,
    jumps_in_nest: u16,
    tracker: HumpTracker,
}

impl ComplexityState {
    fn new() -> Self {
        Self {
            cx: 1,
            ccx: 0,
            max_nest: 0,
            jumps_in_nest: 0,
            tracker: HumpTracker::new(),
        }
    }
}

fn process_node_complexity(
    node: Node<'_>,
    nesting: u16,
    in_else_if: bool,
    source_bytes: &[u8],
    state: &mut ComplexityState,
) -> (u16, bool) {
    let kind = node.kind();
    let text = node.utf8_text(source_bytes).unwrap_or("");

    if is_cyclomatic_decision(kind) || is_bool_operator(kind, text) {
        state.cx = state.cx.saturating_add(1);
    }

    if is_jump_node(kind) && nesting > 0 {
        state.jumps_in_nest = state.jumps_in_nest.saturating_add(1);
    }

    let mut next_nesting = nesting;
    let mut next_in_else_if = false;

    if is_nesting_control(kind) {
        if in_else_if {
            state.ccx = state.ccx.saturating_add(1);
        } else {
            state.ccx = state.ccx.saturating_add(1 + nesting as u32);
            next_nesting = nesting.saturating_add(1);
        }
        state.tracker.note_nesting(nesting, next_nesting, &node);
    } else if kind == "else_clause" || kind == "elif_clause" {
        next_in_else_if = true;
    }

    state.max_nest = state.max_nest.max(next_nesting);
    (next_nesting, next_in_else_if)
}

/// Compute complexity metrics from a Tree-Sitter AST node.
#[must_use]
pub fn compute_complexity_from_node(
    root: Node<'_>,
    source_bytes: &[u8],
    _lang: Lang,
) -> ComplexityMetrics {
    let mut state = ComplexityState::new();
    let mut stack = vec![(root, 0u16, false)];

    while let Some((node, nesting, in_else_if)) = stack.pop() {
        let (child_nest, child_else_if) =
            process_node_complexity(node, nesting, in_else_if, source_bytes, &mut state);

        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            stack.push((child, child_nest, child_else_if));
        }
    }

    let ev = 1u16
        .saturating_add(state.jumps_in_nest)
        .min(state.cx as u16);
    ComplexityMetrics {
        cx: state.cx,
        ccx: state.ccx,
        max_nest: state.max_nest,
        humps: state.tracker.humps,
        deep: state.tracker.deep_lines,
        ev,
    }
}

/// Compute complexity metrics directly from source code string.
#[must_use]
pub fn compute_complexity(code: &str, lang: Lang) -> ComplexityMetrics {
    let source_bytes = code.as_bytes();
    let reg = crate::ingest::LanguageRegistry::new();
    let Ok(registry) = reg else {
        return estimate_text_complexity(code);
    };

    let Some((ts_lang, _)) = registry.get(lang) else {
        return estimate_text_complexity(code);
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(ts_lang).is_err() {
        return estimate_text_complexity(code);
    }

    let Some(tree) = parser.parse(source_bytes, None) else {
        return estimate_text_complexity(code);
    };

    compute_complexity_from_node(tree.root_node(), source_bytes, lang)
}

fn line_has_decision(trimmed: &str) -> bool {
    const PATTERNS: [&str; 7] = ["if ", "if(", "for ", "while ", "match ", " && ", " || "];
    PATTERNS.iter().any(|&p| trimmed.contains(p))
}

/// Lightweight fallback estimator when grammar is unavailable.
fn estimate_text_complexity(code: &str) -> ComplexityMetrics {
    let mut cx = 1u32;
    let mut ccx = 0u32;
    let mut current_nest = 0u16;
    let mut max_nest = 0u16;
    let mut humps = 0u16;
    let mut deep = 0u16;

    for line in code.lines() {
        let trimmed = line.trim();
        let open_b = line.chars().filter(|&c| c == '{').count() as u16;
        let close_b = line.chars().filter(|&c| c == '}').count() as u16;

        if line_has_decision(trimmed) {
            cx += 1;
            ccx += 1 + current_nest as u32;
        }

        let next_nest = current_nest.saturating_add(open_b).saturating_sub(close_b);
        if current_nest < NESTING_BAR && next_nest >= NESTING_BAR {
            humps += 1;
            deep += 1;
        } else if next_nest >= NESTING_BAR {
            deep += 1;
        }

        current_nest = next_nest;
        max_nest = max_nest.max(current_nest);
    }

    ComplexityMetrics {
        cx,
        ccx,
        max_nest,
        humps,
        deep,
        ev: 1.min(cx as u16),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_text_complexity_fallback() {
        let code = "if true { println!(); }";
        let m = compute_complexity(code, Lang::Unknown);
        assert!(m.cx >= 2);
        assert!(m.ccx >= 1);
    }

    #[test]
    fn test_complexity_linear_function() {
        let code = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let m = compute_complexity(code, Lang::Rust);
        assert_eq!(m.cx, 1);
        assert_eq!(m.ccx, 0);
        assert_eq!(m.humps, 0);
        assert_eq!(m.ev, 1);
    }

    #[test]
    fn test_complexity_nested_if_and_humps() {
        let code = r#"
fn test_func(x: i32) {
    if x > 0 {
        if x > 10 {
            if x > 20 {
                if x > 30 {
                    println!("deep");
                }
            }
        }
    }
}
"#;
        let m = compute_complexity(code, Lang::Rust);
        assert!(m.cx >= 5);
        assert!(m.ccx >= 10);
        assert!(m.max_nest >= 4);
        assert!(m.humps >= 1);
        assert!(m.deep >= 1);
    }
}
