use super::{DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::Lang;

pub struct GoLanguage;
pub static GO_LANG: GoLanguage = GoLanguage;

const GO_STD_MEMBERS: &[&str] = &[
    "Error",
    "String",
    "Read",
    "Write",
    "Close",
    "Flush",
    "Seek",
    "Lock",
    "Unlock",
    "RLock",
    "RUnlock",
    "Add",
    "Done",
    "Wait",
    "Load",
    "Store",
    "Swap",
    "CompareAndSwap",
    "Context",
    "Deadline",
    "Err",
    "Value",
    "Len",
    "Less",
    "MarshalJSON",
    "UnmarshalJSON",
    "Scan",
];

const GO_STD_QUALIFIERS: &[&str] = &[
    "fmt", "os", "io", "strings", "bytes", "strconv", "time", "context", "sync", "atomic",
    "errors", "filepath", "path", "http", "url", "json", "xml", "sort", "slices", "maps", "math",
    "rand", "reflect", "regexp", "bufio", "log", "slog", "testing", "exec", "syscall", "runtime",
    "binary", "hex", "base64", "sha256", "tls", "net", "sql",
];

impl LanguageSemantics for GoLanguage {
    fn tag(&self) -> &'static str {
        "go"
    }

    fn is_generated_file_name(&self, file_name: &str) -> bool {
        file_name.ends_with(".pb.go")
    }
    super::impl_grammar_method!(crate tree_sitter_go::LANGUAGE, "../queries/go/tags.scm");

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

    fn extract_custom_scope(
        &self,
        parent: tree_sitter::Node<'_>,
        _name_node: tree_sitter::Node<'_>,
        source_bytes: &[u8],
    ) -> Option<String> {
        if parent.kind() != "method_declaration" {
            return None;
        }
        let recv_list = parent.child_by_field_name("receiver")?;
        let param = recv_list.named_child(0)?;
        let type_node = param.child_by_field_name("type")?;
        let text = type_node.utf8_text(source_bytes).ok()?;
        let base = text
            .trim_start_matches('*')
            .split('[')
            .next()
            .unwrap_or("")
            .trim();
        (!base.is_empty()).then(|| base.to_string())
    }
    fn std_member_names(&self) -> &'static [&'static str] {
        GO_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        GO_STD_QUALIFIERS
    }
}
