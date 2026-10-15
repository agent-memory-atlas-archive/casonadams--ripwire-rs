use super::{DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::{Lang, SymKind};

pub struct GoLanguage;
pub struct DartLanguage;
pub struct ElixirLanguage;
pub struct ErlangLanguage;
pub struct ZigLanguage;
pub struct SolidityLanguage;

pub static GO_LANG: GoLanguage = GoLanguage;
pub static DART_LANG: DartLanguage = DartLanguage;
pub static ELIXIR_LANG: ElixirLanguage = ElixirLanguage;
pub static ERLANG_LANG: ErlangLanguage = ErlangLanguage;
pub static ZIG_LANG: ZigLanguage = ZigLanguage;
pub static SOLIDITY_LANG: SolidityLanguage = SolidityLanguage;

impl LanguageSemantics for GoLanguage {
    fn tag(&self) -> &'static str {
        "go"
    }

    fn display_color(&self) -> &'static str {
        "#00add8"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Go)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        sym_name.chars().next().is_some_and(|c| c.is_uppercase())
    }

    fn check_node_visibility(&self, def_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
        let Some(name_node) = def_node.child_by_field_name("name") else {
            return false;
        };
        let Ok(text) = name_node.utf8_text(source_bytes) else {
            return false;
        };
        text.chars().next().is_some_and(|c| c.is_uppercase())
    }
}

fn dart_following_body(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let mut sib = node.next_sibling();
    while let Some(s) = sib {
        if s.kind() == "function_body" {
            return Some(s);
        }
        if s.is_named() {
            break;
        }
        sib = s.next_sibling();
    }
    None
}

impl LanguageSemantics for DartLanguage {
    fn tag(&self) -> &'static str {
        "dart"
    }

    fn display_color(&self) -> &'static str {
        "#00b4ab"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Dart)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ByNameConvention
    }

    fn resolve_def_body_and_end<'a>(
        &self,
        def_node: Option<tree_sitter::Node<'a>>,
        default_end: usize,
    ) -> (Option<tree_sitter::Node<'a>>, usize) {
        let Some(dn) = def_node else {
            return (None, default_end);
        };
        if let Some(body) = dart_following_body(dn) {
            return (Some(body), body.end_byte());
        }
        (Some(dn), default_end)
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }
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

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }
}

impl LanguageSemantics for ErlangLanguage {
    fn tag(&self) -> &'static str {
        "erl"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Beam
    }
}

impl LanguageSemantics for ZigLanguage {
    fn tag(&self) -> &'static str {
        "zig"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Zig)
    }
}

impl LanguageSemantics for SolidityLanguage {
    fn tag(&self) -> &'static str {
        "sol"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Solidity)
    }
}
