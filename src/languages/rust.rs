use super::{
    DialectGroup, LanguageSemantics, VisibilityMode, normalize_lexical_path, probe_with_suffixes,
    strip_import_quotes,
};
use crate::domain::{Lang, SymKind};
use std::collections::HashMap;
use std::path::Path;

pub struct RustLanguage;
pub static RUST_LANG: RustLanguage = RustLanguage;

const RUST_BUILTIN_MACROS: &[&str] = &[
    "matches",
    "write",
    "writeln",
    "print",
    "println",
    "eprint",
    "eprintln",
    "format",
    "format_args",
    "vec",
    "panic",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "todo",
    "unimplemented",
    "unreachable",
    "cfg",
    "env",
    "option_env",
    "include",
    "include_str",
    "include_bytes",
    "concat",
    "stringify",
    "line",
    "column",
    "file",
    "module_path",
    "compile_error",
    "thread_local",
];

const RUST_STD_MEMBERS: &[&str] = &[
    "len",
    "is_empty",
    "push",
    "push_str",
    "pop",
    "clear",
    "get",
    "get_mut",
    "insert",
    "remove",
    "contains",
    "contains_key",
    "iter",
    "iter_mut",
    "into_iter",
    "as_slice",
    "as_mut_slice",
    "as_str",
    "as_bytes",
    "as_ref",
    "as_mut",
    "as_deref",
    "as_deref_mut",
    "clone",
    "to_string",
    "to_owned",
    "to_vec",
    "unwrap",
    "unwrap_or",
    "unwrap_or_default",
    "unwrap_or_else",
    "expect",
    "ok",
    "err",
    "map",
    "and_then",
    "or_else",
    "filter",
    "collect",
    "extend",
    "reserve",
    "with_capacity",
    "default",
    "from",
    "into",
    "fmt",
    "eq",
    "cmp",
    "partial_cmp",
    "hash",
    "index",
    "deref",
    "next",
];

const RUST_STD_QUALIFIERS: &[&str] = &[
    "std",
    "core",
    "alloc",
    "Vec",
    "String",
    "Option",
    "Result",
    "Box",
    "Arc",
    "Rc",
    "Mutex",
    "RwLock",
    "HashMap",
    "HashSet",
    "BTreeMap",
    "BTreeSet",
    "VecDeque",
    "Path",
    "PathBuf",
    "OsStr",
    "OsString",
    "CStr",
    "CString",
    "File",
    "Command",
    "Instant",
    "Duration",
    "SystemTime",
    "env",
    "fs",
    "io",
    "mem",
    "ptr",
    "slice",
    "str",
    "fmt",
    "iter",
    "cmp",
    "ops",
    "sync",
    "thread",
];

impl LanguageSemantics for RustLanguage {
    fn tag(&self) -> &'static str {
        "rs"
    }
    super::impl_grammar_method!(crate tree_sitter_rust::LANGUAGE, "../queries/rust/tags.scm");

    fn has_lifetime_tick_syntax(&self) -> bool {
        true
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
        is_macro_inv && RUST_BUILTIN_MACROS.contains(&name)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        RUST_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        RUST_STD_QUALIFIERS
    }

    fn symbol_implied_import(&self, kind: SymKind, name: &str) -> Option<String> {
        (kind == SymKind::Section).then(|| format!("self::{name}"))
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

fn resolve_rust_super_dir(importer_path: &str) -> &str {
    let p = Path::new(importer_path);
    let is_mod_root = matches!(
        p.file_name().and_then(|n| n.to_str()),
        Some("mod.rs" | "lib.rs" | "main.rs")
    );
    let dir = if is_mod_root {
        p.parent().and_then(|d| d.parent())
    } else {
        p.parent()
    };
    dir.and_then(|d| d.to_str()).unwrap_or("")
}

fn split_rust_anchor(clean: &str) -> Option<(&str, &str)> {
    for anchor in ["crate", "self", "super"] {
        if clean == anchor {
            return Some((anchor, ""));
        }
        if let Some(rest) = clean
            .strip_prefix(anchor)
            .and_then(|s| s.strip_prefix("::"))
        {
            return Some((anchor, rest));
        }
    }
    None
}

fn resolve_rust_base_and_rest<'a>(
    importer_path: &'a str,
    clean: &'a str,
) -> Option<(&'a str, &'a str)> {
    let (anchor, rest) = split_rust_anchor(clean)?;
    let base = match anchor {
        "crate" => find_rust_crate_src_dir(importer_path),
        "self" => Path::new(importer_path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or(""),
        _ => resolve_rust_super_dir(importer_path),
    };
    Some((base, rest))
}

fn probe_rust_segs_in_base(
    base_dir: &str,
    segs: &[&str],
    file_index: &HashMap<&str, u32>,
) -> Option<u32> {
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

fn resolve_rust_workspace_crate_target(
    clean: &str,
    file_index: &HashMap<&str, u32>,
) -> Option<u32> {
    let segs: Vec<&str> = clean.split("::").filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 {
        return None;
    }
    let crate_dir = format!("crates/{}/src", segs[0].replace('_', "-"));
    probe_rust_segs_in_base(&crate_dir, &segs[1..], file_index)
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
    let segs: Vec<&str> = clean.split("::").filter(|s| !s.is_empty()).collect();
    if let Some((base_dir, rest)) = resolve_rust_base_and_rest(importer_path, clean) {
        let rest_segs: Vec<&str> = rest.split("::").filter(|s| !s.is_empty()).collect();
        return probe_rust_segs_in_base(base_dir, &rest_segs, file_index);
    }
    let self_dir = Path::new(importer_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    for base in [self_dir, find_rust_crate_src_dir(importer_path)] {
        if let Some(id) = probe_rust_segs_in_base(base, &segs, file_index) {
            return Some(id);
        }
    }
    resolve_rust_workspace_crate_target(clean, file_index)
}
