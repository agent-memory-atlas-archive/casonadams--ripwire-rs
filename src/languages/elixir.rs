use super::{DefCaptureGateContext, DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::SymKind;

pub struct ElixirLanguage;
pub static ELIXIR_LANG: ElixirLanguage = ElixirLanguage;

pub(super) const BEAM_STD_MEMBERS: &[&str] = &[
    "get",
    "put",
    "delete",
    "fetch",
    "fetch!",
    "merge",
    "keys",
    "values",
    "has_key?",
    "new",
    "to_list",
    "from_struct",
    "update",
    "update!",
    "pop",
    "take",
    "drop",
    "split",
    "map",
    "each",
    "filter",
    "reject",
    "reduce",
    "find",
    "any?",
    "all?",
    "member?",
    "count",
    "empty?",
    "concat",
    "flat_map",
    "join",
    "sort",
    "reverse",
    "uniq",
    "at",
    "first",
    "last",
    "trim",
    "downcase",
    "upcase",
    "replace",
    "starts_with?",
    "ends_with?",
    "contains?",
    "inspect",
    "puts",
    "info",
    "debug",
    "warn",
    "warning",
    "error",
];

pub(super) const BEAM_STD_QUALIFIERS: &[&str] = &[
    "Enum",
    "Map",
    "Keyword",
    "List",
    "String",
    "Integer",
    "Float",
    "Tuple",
    "MapSet",
    "Stream",
    "File",
    "Path",
    "IO",
    "Process",
    "Task",
    "Agent",
    "GenServer",
    "Supervisor",
    "Application",
    "Logger",
    "Kernel",
    "System",
    "DateTime",
    "NaiveDateTime",
    "Date",
    "Time",
    "URI",
    "Regex",
    "Base",
    "Jason",
    "ets",
    "erlang",
    "lists",
    "maps",
    "proplists",
    "binary",
    "io",
    "file",
    "timer",
];

const ELIXIR_FN_KEYWORDS: &[&str] = &[
    "def",
    "defp",
    "defmacro",
    "defmacrop",
    "defguard",
    "defguardp",
    "defdelegate",
];

const ELIXIR_TYPED_ATTRS: &[&str] = &["type", "typep", "opaque", "callback", "macrocallback"];

const ELIXIR_METADATA_ATTRS: &[&str] = &[
    "doc",
    "moduledoc",
    "typedoc",
    "spec",
    "impl",
    "behaviour",
    "behavior",
    "derive",
    "enforce_keys",
    "dialyzer",
    "on_load",
    "before_compile",
    "after_compile",
    "after_verify",
    "optional_callbacks",
    "compile",
    "external_resource",
    "vsn",
    "file",
];

const ELIXIR_SPECIAL_CALLS: &[&str] = &[
    "defmodule",
    "defprotocol",
    "defimpl",
    "defstruct",
    "defexception",
    "defoverridable",
    "alias",
    "import",
    "require",
    "use",
    "quote",
    "unquote",
    "unquote_splicing",
    "case",
    "cond",
    "for",
    "if",
    "unless",
    "with",
    "receive",
    "try",
    "test",
];

fn is_elixir_call_def_allowed(dn: tree_sitter::Node<'_>, ctx: &DefCaptureGateContext<'_>) -> bool {
    let Some(target) = dn
        .child_by_field_name("target")
        .and_then(|t| t.utf8_text(ctx.source_bytes).ok())
    else {
        return false;
    };
    if ctx.kind == SymKind::Section {
        return matches!(target, "defmodule" | "defprotocol" | "defimpl");
    }
    if target == "test" {
        return ctx.name_node.is_some_and(|n| n.kind() == "string");
    }
    ELIXIR_FN_KEYWORDS.contains(&target) && !matches!(ctx.name, "unquote" | "when")
}

fn is_elixir_attr_def_allowed(dn: tree_sitter::Node<'_>, ctx: &DefCaptureGateContext<'_>) -> bool {
    if ctx.sub == Some("attribute") {
        return !ELIXIR_TYPED_ATTRS.contains(&ctx.name)
            && !ELIXIR_METADATA_ATTRS.contains(&ctx.name);
    }
    dn.child_by_field_name("operand")
        .and_then(|op| op.child_by_field_name("target"))
        .and_then(|t| t.utf8_text(ctx.source_bytes).ok())
        .is_some_and(|t| ELIXIR_TYPED_ATTRS.contains(&t))
}

impl LanguageSemantics for ElixirLanguage {
    fn tag(&self) -> &'static str {
        "ex"
    }

    fn display_color(&self) -> &'static str {
        "#6e4a7e"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Beam
    }

    fn uses_hash_comments(&self) -> bool {
        true
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn resolve_def_params_and_name(
        &self,
        def_node: Option<tree_sitter::Node<'_>>,
        source_bytes: &[u8],
        kind: SymKind,
        orig_name: &str,
    ) -> (u16, String) {
        let params = def_node
            .map(|n| crate::ingest::elixir_param_count(n, source_bytes))
            .unwrap_or(0);
        let name = if kind == SymKind::Function {
            format!("{orig_name}/{params}")
        } else {
            orig_name.to_string()
        };
        (params, name)
    }

    fn format_callee_name(&self, name: &str, arg_count: u16, arg_count_known: bool) -> String {
        let arity = if arg_count_known { arg_count } else { 0 };
        format!("{name}/{arity}")
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        let Some(dn) = ctx.def_node else {
            return false;
        };
        match dn.kind() {
            "call" => is_elixir_call_def_allowed(dn, ctx),
            "unary_operator" => is_elixir_attr_def_allowed(dn, ctx),
            _ => true,
        }
    }

    fn should_ignore_call(&self, _name_node: Option<tree_sitter::Node<'_>>, name: &str) -> bool {
        ELIXIR_FN_KEYWORDS.contains(&name) || ELIXIR_SPECIAL_CALLS.contains(&name)
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        let bare = name.split('/').next().unwrap_or(name);
        BEAM_STD_MEMBERS.contains(&bare)
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        BEAM_STD_QUALIFIERS
    }
}
