/// Grammar node kinds a language contributes to `cx` / `ccx` complexity.
///
/// Only named nodes are matched, so keyword tokens never double-count the statement they belong to.
#[derive(Debug, Clone, Copy)]
pub struct ComplexityKinds {
    /// Nodes that add one linear path to `cx` without affecting `ccx` (`case`, `match_arm`, ...).
    pub paths: &'static [&'static str],
    /// Nodes that add one linear path to `cx` and one to `ccx` regardless of nesting (`elif_clause`).
    pub flat: &'static [&'static str],
    /// Control-flow nodes that add one path to `cx` and `1 + nesting` to `ccx`, then deepen nesting.
    pub nesting: &'static [&'static str],
    /// Nodes that add `1 + nesting` to `ccx` and deepen nesting but are not a path themselves (`switch`).
    pub nest_only: &'static [&'static str],
    /// Nodes (named or keyword tokens) introducing an `else` branch.
    pub else_kinds: &'static [&'static str],
    /// Wrapper nodes between an `else` token and its statement (Kotlin `control_structure_body`).
    pub else_bodies: &'static [&'static str],
    /// Early-exit nodes counted toward the essential-complexity jump count.
    pub jumps: &'static [&'static str],
    /// Dedicated short-circuit nodes whose kind is the operator (`logical_and_expression`).
    pub logical_nodes: &'static [&'static str],
    /// Binary-expression nodes whose operator token may be a short-circuit operator.
    pub binary_nodes: &'static [&'static str],
    /// Operator spellings that short-circuit inside `binary_nodes`.
    pub logical_ops: &'static [&'static str],
}

impl ComplexityKinds {
    /// A language with no executable control flow.
    pub const NONE: Self = Self {
        paths: &[],
        flat: &[],
        nesting: &[],
        nest_only: &[],
        else_kinds: &[],
        else_bodies: &[],
        jumps: &[],
        logical_nodes: &[],
        binary_nodes: &[],
        logical_ops: &[],
    };

    /// Base shared by grammars following the C statement vocabulary; languages override by struct update.
    pub const C_STYLE: Self = Self {
        paths: &["case_statement"],
        flat: &[],
        nesting: &[
            "if_statement",
            "for_statement",
            "while_statement",
            "do_statement",
            "catch_clause",
            "conditional_expression",
        ],
        nest_only: &["switch_statement"],
        else_kinds: &["else", "else_clause"],
        else_bodies: &[],
        jumps: &[
            "break_statement",
            "continue_statement",
            "goto_statement",
            "return_statement",
            "throw_statement",
        ],
        logical_nodes: &[],
        binary_nodes: &["binary_expression"],
        logical_ops: &["&&", "||"],
    };
}
