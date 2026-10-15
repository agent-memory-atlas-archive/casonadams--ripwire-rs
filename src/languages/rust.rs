use super::{
    DialectGroup, LanguageSemantics, VisibilityMode, normalize_lexical_path, probe_with_suffixes,
    strip_import_quotes,
};
use crate::domain::Lang;
use std::collections::HashMap;
use std::path::Path;

pub struct RustLanguage;
pub static RUST_LANG: RustLanguage = RustLanguage;

impl LanguageSemantics for RustLanguage {
    fn tag(&self) -> &'static str {
        "rs"
    }

    fn display_color(&self) -> &'static str {
        "#dea584"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Isolated(Lang::Rust)
    }

    fn visibility_mode(&self) -> VisibilityMode {
        VisibilityMode::ExplicitModifier
    }

    fn should_ignore_call(&self, name_node: Option<tree_sitter::Node<'_>>, name: &str) -> bool {
        let is_macro_inv = name_node
            .and_then(|n| n.parent())
            .is_some_and(|p| p.kind() == "macro_invocation");
        is_macro_inv && is_rust_builtin_macro_name(name)
    }

    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        resolve_rust_use_target(importer_path, raw_target, file_index)
    }
}

fn is_rust_builtin_macro_name(name: &str) -> bool {
    matches!(
        name,
        "matches"
            | "write"
            | "writeln"
            | "print"
            | "println"
            | "eprint"
            | "eprintln"
            | "format"
            | "format_args"
            | "vec"
            | "panic"
            | "assert"
            | "assert_eq"
            | "assert_ne"
            | "debug_assert"
            | "debug_assert_eq"
            | "debug_assert_ne"
            | "todo"
            | "unimplemented"
            | "unreachable"
            | "cfg"
            | "env"
            | "option_env"
            | "include"
            | "include_str"
            | "include_bytes"
            | "concat"
            | "stringify"
            | "line"
            | "column"
            | "file"
            | "module_path"
            | "compile_error"
            | "thread_local"
    )
}

fn find_rust_crate_src_dir(importer_path: &str) -> &str {
    if let Some(pos) = importer_path.rfind("/src/") {
        &importer_path[..pos + 4]
    } else if importer_path.starts_with("src/") {
        "src"
    } else {
        Path::new(importer_path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
    }
}

fn resolve_rust_base_and_rest<'a>(
    importer_path: &'a str,
    clean: &'a str,
) -> Option<(&'a str, &'a str)> {
    if let Some(r) = clean.strip_prefix("crate::") {
        Some((find_rust_crate_src_dir(importer_path), r))
    } else if let Some(r) = clean.strip_prefix("self::") {
        let dir = Path::new(importer_path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        Some((dir, r))
    } else if let Some(r) = clean.strip_prefix("super::") {
        let parent = Path::new(importer_path)
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.to_str())
            .unwrap_or("");
        Some((parent, r))
    } else {
        None
    }
}

fn resolve_rust_use_target(
    importer_path: &str,
    raw_target: &str,
    file_index: &HashMap<&str, u32>,
) -> Option<u32> {
    let clean = strip_import_quotes(raw_target)
        .split('{')
        .next()
        .unwrap_or("")
        .trim_end_matches("::*")
        .trim_end_matches("::");
    let (base_dir, rest) = resolve_rust_base_and_rest(importer_path, clean)?;
    let segs: Vec<&str> = rest.split("::").filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        return None;
    }
    for take_len in [segs.len(), segs.len().saturating_sub(1)] {
        if take_len == 0 {
            continue;
        }
        let rel = format!("./{}", segs[..take_len].join("/"));
        let cand = normalize_lexical_path(base_dir, &rel);
        if let Some(id) = probe_with_suffixes(&cand, file_index, &[".rs", "/mod.rs"]) {
            return Some(id);
        }
    }
    None
}
