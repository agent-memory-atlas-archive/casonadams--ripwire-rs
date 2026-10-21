/// Grammar node kinds the language-neutral extraction, slicing, and contract passes rely on.
#[derive(Debug, Clone, Copy)]
pub struct SyntaxKinds {
    /// Named definitions that scope their members (class, trait, module, namespace).
    pub scope_containers: &'static [&'static str],
    /// Qualified names whose `scope`/`path` field qualifies the last segment (`a::b`, `A::m`).
    pub scoped_paths: &'static [&'static str],
    /// Member accesses whose receiver qualifies the accessed name.
    pub member_access: &'static [&'static str],
    /// Member accesses whose receiver is the `object`/`argument`/`value` field.
    pub field_access: &'static [&'static str],
    /// Nodes that open a definition body.
    pub body_blocks: &'static [&'static str],
    /// Nodes a definition's body may be nested inside (declarations, callables, casts).
    pub body_wrappers: &'static [&'static str],
    /// Function definitions whose `name` field names the function.
    pub function_defs: &'static [&'static str],
    /// Declarators whose `declarator` field names a function of an enclosing `function_defs` node.
    pub function_declarators: &'static [&'static str],
    /// Local binding forms besides `*declaration*` / `*declarator*` kinds.
    pub local_bindings: &'static [&'static str],
    /// Declarations that bind a type or callable rather than a local.
    pub non_local_declarations: &'static [&'static str],
    /// Destructuring and declarator wrappers around a bound name (`(a, b)`, `*p`, `{ x = 1 }`).
    pub binding_patterns: &'static [&'static str],
    /// Identifier kinds that name a local.
    pub identifiers: &'static [&'static str],
    /// Contexts naming types rather than values.
    pub type_contexts: &'static [&'static str],
    /// Callable values whose parameter list a definition's contract counts.
    pub callables: &'static [&'static str],
    /// Nodes between a definition and its callable value.
    pub callable_wrappers: &'static [&'static str],
    /// Definitions whose `*signature*` child carries the parameter list.
    pub signature_holders: &'static [&'static str],
    /// Parameter lists in which only `parameter` children are parameters.
    pub strict_param_lists: &'static [&'static str],
    /// Bracketed groups of optional parameters counted child by child.
    pub optional_param_groups: &'static [&'static str],
}

impl SyntaxKinds {
    pub const NONE: Self = Self {
        scope_containers: &[],
        scoped_paths: &[],
        member_access: &[],
        field_access: &[],
        body_blocks: &[],
        body_wrappers: &[],
        function_defs: &[],
        function_declarators: &[],
        local_bindings: &[],
        non_local_declarations: &[],
        binding_patterns: &[],
        identifiers: &[],
        type_contexts: &[],
        callables: &[],
        callable_wrappers: &[],
        signature_holders: &[],
        strict_param_lists: &[],
        optional_param_groups: &[],
    };
}
