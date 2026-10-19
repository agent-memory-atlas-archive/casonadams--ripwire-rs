use super::{DefCaptureGateContext, DialectGroup, LanguageSemantics, VisibilityMode};

pub struct JavaLanguage;
pub static JAVA_LANG: JavaLanguage = JavaLanguage;

pub(super) const JVM_STD_MEMBERS: &[&str] = &[
    "get",
    "set",
    "put",
    "remove",
    "add",
    "addAll",
    "clear",
    "contains",
    "containsKey",
    "size",
    "isEmpty",
    "iterator",
    "stream",
    "map",
    "filter",
    "flatMap",
    "forEach",
    "collect",
    "toList",
    "toArray",
    "orElse",
    "orElseGet",
    "orElseThrow",
    "isPresent",
    "equals",
    "hashCode",
    "toString",
    "compareTo",
    "length",
    "charAt",
    "substring",
    "split",
    "trim",
    "replace",
    "startsWith",
    "endsWith",
    "toLowerCase",
    "toUpperCase",
    "format",
    "valueOf",
    "parseInt",
    "parseLong",
    "append",
];

pub(super) const JVM_STD_QUALIFIERS: &[&str] = &[
    "System",
    "Math",
    "String",
    "Integer",
    "Long",
    "Double",
    "Boolean",
    "Objects",
    "Collections",
    "Arrays",
    "Optional",
    "Stream",
    "Collectors",
    "List",
    "Map",
    "Set",
    "Files",
    "Paths",
    "Path",
    "UUID",
    "Instant",
    "Duration",
    "LocalDate",
    "LocalDateTime",
    "CompletableFuture",
    "Thread",
    "Executors",
    "LoggerFactory",
];

impl LanguageSemantics for JavaLanguage {
    fn tag(&self) -> &'static str {
        "java"
    }
    super::impl_grammar_method!(crate tree_sitter_java::LANGUAGE, "../queries/java/tags.scm");

    fn display_color(&self) -> &'static str {
        "#b07219"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Jvm
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
        kind.is_callable() && !super::is_non_entrypoint_path(rel_path) && sym_name == "main"
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        ctx.sub != Some("constant") || super::is_screaming_snake(ctx.name)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        JVM_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        JVM_STD_QUALIFIERS
    }
}
