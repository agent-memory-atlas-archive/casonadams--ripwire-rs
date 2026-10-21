use super::{
    ComplexityKinds, DefCaptureGateContext, DialectGroup, LanguageSemantics, VisibilityMode,
};
use crate::domain::Lang;

pub struct CSharpLanguage;
pub static CSHARP_LANG: CSharpLanguage = CSharpLanguage;

const CSHARP_STD_MEMBERS: &[&str] = &[
    "Add",
    "AddRange",
    "Remove",
    "Clear",
    "Contains",
    "ContainsKey",
    "TryGetValue",
    "ToArray",
    "ToList",
    "ToDictionary",
    "ToHashSet",
    "Select",
    "Where",
    "FirstOrDefault",
    "SingleOrDefault",
    "Any",
    "All",
    "Count",
    "ToString",
    "Equals",
    "GetHashCode",
    "GetType",
    "Substring",
    "Split",
    "Trim",
    "Replace",
    "StartsWith",
    "EndsWith",
    "ToLower",
    "ToUpper",
    "Format",
    "Parse",
    "TryParse",
    "Append",
    "Dispose",
    "ConfigureAwait",
];

const CSHARP_STD_QUALIFIERS: &[&str] = &[
    "Console",
    "Math",
    "String",
    "Int32",
    "Int64",
    "Double",
    "Boolean",
    "Guid",
    "DateTime",
    "DateTimeOffset",
    "TimeSpan",
    "Task",
    "Enumerable",
    "Array",
    "Convert",
    "Environment",
    "File",
    "Directory",
    "Path",
    "JsonSerializer",
    "Regex",
    "Interlocked",
    "GC",
];

const KINDS: ComplexityKinds = ComplexityKinds {
    paths: &["switch_section", "switch_expression_arm"],
    flat: &[],
    nesting: &[
        "if_statement",
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_statement",
        "catch_clause",
        "conditional_expression",
    ],
    nest_only: &["switch_statement", "switch_expression"],
    else_kinds: &["else"],
    else_bodies: &[],
    jumps: &[
        "break_statement",
        "continue_statement",
        "goto_statement",
        "return_statement",
        "throw_statement",
        "throw_expression",
    ],
    logical_nodes: &[],
    binary_nodes: &["binary_expression"],
    logical_ops: &["&&", "||", "??"],
};

impl LanguageSemantics for CSharpLanguage {
    fn complexity_kinds(&self) -> &'static ComplexityKinds {
        &KINDS
    }

    fn tag(&self) -> &'static str {
        "cs"
    }
    super::impl_grammar_method!(crate tree_sitter_c_sharp::LANGUAGE, "../queries/csharp/tags.scm");

    fn lint_return_type<'a>(&self, sig: &'a str, name: &str) -> Option<&'a str> {
        super::lint_hooks::prefix_return_type(sig, name)
    }

    fn type_declaration_keywords(&self) -> &'static [&'static str] {
        &["enum"]
    }

    fn camel_test_partner_subject<'a>(&self, file_stem: &'a str) -> Option<&'a str> {
        super::strip_camel_test_suffix(file_stem)
    }

    fn display_color(&self) -> &'static str {
        "#178600"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::CSharp)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn is_entrypoint_symbol(
        &self,
        sym_name: &str,
        _scope: Option<&str>,
        kind: crate::domain::SymKind,
        rel_path: &str,
    ) -> bool {
        kind.is_callable()
            && !super::is_non_entrypoint_path(rel_path)
            && (sym_name == "main" || sym_name == "Main")
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        ctx.sub != Some("constant") || super::is_screaming_snake(ctx.name)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        CSHARP_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        CSHARP_STD_QUALIFIERS
    }
}
