use crate::domain::Lang;
use crate::languages::ComplexityKinds;
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

struct ComplexityState<'a> {
    source: &'a [u8],
    cx: u32,
    ccx: u32,
    max_nest: u16,
    jumps_in_nest: u16,
    tracker: HumpTracker,
}

impl<'a> ComplexityState<'a> {
    fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            cx: 1,
            ccx: 0,
            max_nest: 0,
            jumps_in_nest: 0,
            tracker: HumpTracker::new(),
        }
    }
}

fn kind_of(node: Option<Node<'_>>) -> &'static str {
    node.map_or("", |n| n.kind())
}

fn is_else_if(node: Node<'_>, kinds: &ComplexityKinds) -> bool {
    let after_else = |n: Option<Node<'_>>| kinds.else_kinds.contains(&kind_of(n));
    let parent = node.parent();
    after_else(parent)
        || after_else(node.prev_sibling())
        || parent
            .is_some_and(|p| kinds.else_bodies.contains(&p.kind()) && after_else(p.prev_sibling()))
}

fn opens_else_if(node: Node<'_>, kinds: &ComplexityKinds) -> bool {
    let mut cursor = node.walk();
    let in_body = node
        .named_children(&mut cursor)
        .any(|c| kinds.nesting.contains(&c.kind()));
    let next = node.next_sibling();
    let in_next_body = next.is_some_and(|n| {
        kinds.else_bodies.contains(&n.kind()) && {
            let mut cursor = n.walk();
            n.named_children(&mut cursor)
                .any(|c| kinds.nesting.contains(&c.kind()))
        }
    });
    in_body || in_next_body || kinds.nesting.contains(&kind_of(next))
}

fn logical_op<'a>(node: Node<'_>, kinds: &ComplexityKinds, source: &'a [u8]) -> Option<&'a str> {
    let kind = node.kind();
    if kinds.logical_nodes.contains(&kind) {
        return Some(kind);
    }
    if !kinds.binary_nodes.contains(&kind) {
        return None;
    }
    let spelled = node
        .child_by_field_name("operator")
        .and_then(|op| op.utf8_text(source).ok())
        .filter(|text| kinds.logical_ops.contains(text));
    spelled.or_else(|| {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .map(|c| c.kind())
            .find(|k| kinds.logical_ops.contains(k))
    })
}

fn note_logical(node: Node<'_>, kinds: &ComplexityKinds, state: &mut ComplexityState) {
    let Some(op) = logical_op(node, kinds, state.source) else {
        return;
    };
    state.cx = state.cx.saturating_add(1);
    let continues = node
        .parent()
        .and_then(|p| logical_op(p, kinds, state.source))
        .is_some_and(|parent_op| parent_op == op);
    if !continues {
        state.ccx = state.ccx.saturating_add(1);
    }
}

fn note_else(node: Node<'_>, kinds: &ComplexityKinds, state: &mut ComplexityState) {
    if !opens_else_if(node, kinds) {
        state.ccx = state.ccx.saturating_add(1);
    }
}

fn process_node_complexity(
    node: Node<'_>,
    nesting: u16,
    kinds: &ComplexityKinds,
    state: &mut ComplexityState,
) -> u16 {
    let kind = node.kind();
    if kinds.else_kinds.contains(&kind) {
        note_else(node, kinds, state);
    }
    if !node.is_named() {
        return nesting;
    }
    note_logical(node, kinds, state);
    if kinds.paths.contains(&kind) {
        state.cx = state.cx.saturating_add(1);
    }
    if kinds.flat.contains(&kind) {
        state.cx = state.cx.saturating_add(1);
        state.ccx = state.ccx.saturating_add(1);
    }
    if kinds.jumps.contains(&kind) && nesting > 0 {
        state.jumps_in_nest = state.jumps_in_nest.saturating_add(1);
    }
    let counts_path = kinds.nesting.contains(&kind);
    if !counts_path && !kinds.nest_only.contains(&kind) {
        return nesting;
    }
    if counts_path {
        state.cx = state.cx.saturating_add(1);
    }
    let next_nesting = if counts_path && is_else_if(node, kinds) {
        state.ccx = state.ccx.saturating_add(1);
        nesting
    } else {
        state.ccx = state.ccx.saturating_add(1 + nesting as u32);
        nesting.saturating_add(1)
    };
    state.tracker.note_nesting(nesting, next_nesting, &node);
    state.max_nest = state.max_nest.max(next_nesting);
    next_nesting
}

/// Compute complexity metrics from a Tree-Sitter AST node.
#[must_use]
pub fn compute_complexity_from_node(
    root: Node<'_>,
    source_bytes: &[u8],
    lang: Lang,
) -> ComplexityMetrics {
    let kinds = lang.semantics().complexity_kinds();
    let mut state = ComplexityState::new(source_bytes);
    let mut stack = vec![(root, 0u16)];

    while let Some((node, nesting)) = stack.pop() {
        let child_nest = process_node_complexity(node, nesting, kinds, &mut state);
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            stack.push((child, child_nest));
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
