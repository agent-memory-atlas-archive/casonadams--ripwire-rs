use super::{DefCaptureGateContext, DialectGroup, LanguageSemantics, VisibilityMode};
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

impl LanguageSemantics for CSharpLanguage {
    fn tag(&self) -> &'static str {
        "cs"
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
