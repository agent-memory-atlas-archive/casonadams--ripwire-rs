use super::{DefCaptureGateContext, DialectGroup, LanguageSemantics, VisibilityMode};
use crate::domain::{Lang, SymKind};
use std::collections::HashMap;

pub struct DartLanguage;
pub static DART_LANG: DartLanguage = DartLanguage;

const DART_STD_MEMBERS: &[&str] = &[
    "add",
    "addAll",
    "remove",
    "clear",
    "contains",
    "containsKey",
    "map",
    "where",
    "expand",
    "forEach",
    "fold",
    "reduce",
    "toList",
    "toSet",
    "join",
    "split",
    "trim",
    "substring",
    "replaceAll",
    "startsWith",
    "endsWith",
    "toLowerCase",
    "toUpperCase",
    "toString",
    "compareTo",
    "then",
    "catchError",
    "whenComplete",
    "isAfter",
    "isBefore",
    "isAtSameMomentAs",
    "difference",
    "toUtc",
    "toLocal",
    "toIso8601String",
    "firstWhere",
    "singleWhere",
    "any",
    "every",
    "take",
    "skip",
    "cast",
    "asMap",
    "putIfAbsent",
    "update",
    "removeWhere",
    "wait",
    "value",
    "error",
    "delayed",
    "decode",
    "encode",
];

const DART_STD_QUALIFIERS: &[&str] = &[
    "Future",
    "Stream",
    "List",
    "Map",
    "Set",
    "Iterable",
    "String",
    "int",
    "double",
    "num",
    "bool",
    "DateTime",
    "Duration",
    "Uri",
    "RegExp",
    "JsonCodec",
    "json",
    "utf8",
    "math",
    "Platform",
    "File",
    "Directory",
];

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

fn resolve_dart_package_import(
    importer_path: &str,
    pkg_spec: &str,
    file_index: &HashMap<&str, u32>,
) -> Option<u32> {
    let (pkg, subpath) = pkg_spec.split_once('/')?;
    if let Some((top_dir, _)) = importer_path.split_once('/') {
        let ws_cand = format!("{top_dir}/{pkg}/lib/{subpath}");
        if let Some(id) = super::probe_with_suffixes(&ws_cand, file_index, &[".dart"]) {
            return Some(id);
        }
    }
    for cand in [
        format!("packages/{pkg}/lib/{subpath}"),
        format!("{pkg}/lib/{subpath}"),
        format!("lib/{subpath}"),
    ] {
        if let Some(id) = super::probe_with_suffixes(&cand, file_index, &[".dart"]) {
            return Some(id);
        }
    }
    None
}

impl LanguageSemantics for DartLanguage {
    fn tag(&self) -> &'static str {
        "dart"
    }
    super::impl_grammar_method!(ffi tree_sitter_dart, include_str!("../queries/dart/tags.scm"));

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
        def_node
            .and_then(dart_following_body)
            .map_or((def_node, default_end), |b| (Some(b), b.end_byte()))
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        !(ctx.kind == SymKind::Function
            && ctx
                .def_node
                .and_then(|dn| dn.parent())
                .is_some_and(|p| p.kind() == "method_signature"))
    }

    fn is_symbol_name_public(&self, sym_name: &str) -> bool {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    }

    fn should_ignore_call(&self, _name_node: Option<tree_sitter::Node<'_>>, name: &str) -> bool {
        matches!(name, "print" | "identical" | "jsonEncode" | "jsonDecode")
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        DART_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        DART_STD_QUALIFIERS
    }

    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        let clean = super::strip_import_quotes(raw_target);
        if clean.is_empty() || clean.starts_with("dart:") {
            return None;
        }
        if let Some(pkg_spec) = clean.strip_prefix("package:") {
            return resolve_dart_package_import(importer_path, pkg_spec, file_index);
        }
        super::resolve_relative_or_exact(importer_path, clean, file_index, &[".dart"])
    }
}
