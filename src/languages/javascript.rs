use super::{
    DefCaptureGateContext, DialectGroup, LanguageSemantics, probe_with_suffixes,
    resolve_relative_or_exact, strip_import_quotes,
};
use std::collections::HashMap;

pub struct JavaScriptLanguage;
pub static JAVASCRIPT_LANG: JavaScriptLanguage = JavaScriptLanguage;

const WEB_SUFFIXES: &[&str] = &[
    ".ts",
    ".tsx",
    "/index.ts",
    "/index.tsx",
    ".js",
    ".jsx",
    "/index.js",
    ".d.ts",
];

pub(super) const JS_STD_MEMBERS: &[&str] = &[
    "push",
    "pop",
    "shift",
    "unshift",
    "slice",
    "splice",
    "map",
    "filter",
    "reduce",
    "forEach",
    "find",
    "findIndex",
    "includes",
    "indexOf",
    "join",
    "split",
    "trim",
    "replace",
    "replaceAll",
    "startsWith",
    "endsWith",
    "toLowerCase",
    "toUpperCase",
    "toString",
    "valueOf",
    "keys",
    "values",
    "entries",
    "then",
    "catch",
    "finally",
    "test",
    "match",
    "has",
    "get",
    "set",
    "delete",
    "add",
    "clear",
    "some",
    "every",
    "flat",
    "flatMap",
    "sort",
    "reverse",
    "concat",
    "fill",
    "at",
    "charAt",
    "charCodeAt",
    "codePointAt",
    "padStart",
    "padEnd",
    "repeat",
    "search",
    "matchAll",
    "exec",
    "toFixed",
    "toISOString",
    "getTime",
    "resolve",
    "reject",
    "all",
    "allSettled",
    "race",
    "any",
    "from",
    "of",
    "isArray",
    "assign",
    "freeze",
    "create",
    "defineProperty",
    "hasOwn",
    "hasOwnProperty",
    "parse",
    "stringify",
    "now",
    "Date",
    "dirname",
    "basename",
    "extname",
    "relative",
    "isAbsolute",
];

pub(super) const JS_STD_QUALIFIERS: &[&str] = &[
    "Array",
    "Object",
    "JSON",
    "Math",
    "Number",
    "String",
    "Boolean",
    "Symbol",
    "BigInt",
    "Date",
    "RegExp",
    "Promise",
    "Reflect",
    "Proxy",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "Error",
    "TypeError",
    "RangeError",
    "SyntaxError",
    "Uint8Array",
    "Int32Array",
    "Float64Array",
    "ArrayBuffer",
    "DataView",
    "Buffer",
    "console",
    "process",
    "URL",
    "URLSearchParams",
    "Intl",
    "crypto",
    "TextEncoder",
    "TextDecoder",
    "Performance",
    "performance",
    "navigator",
    "window",
    "document",
    "globalThis",
    "fs",
    "path",
    "os",
    "url",
    "util",
    "child_process",
    "Bun",
    "Deno",
];

fn is_cjs_export_target(name_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
    let Some(obj) = name_node
        .parent()
        .and_then(|m| m.child_by_field_name("object"))
    else {
        return false;
    };
    if obj.kind() == "identifier" {
        return obj.utf8_text(source_bytes) == Ok("exports");
    }
    if obj.kind() == "member_expression" {
        let is_mod = obj
            .child_by_field_name("object")
            .is_some_and(|o| o.kind() == "identifier" && o.utf8_text(source_bytes) == Ok("module"));
        let is_exp = obj
            .child_by_field_name("property")
            .is_some_and(|p| p.utf8_text(source_bytes) == Ok("exports"));
        return is_mod && is_exp;
    }
    false
}

fn is_prototype_member_target(name_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
    name_node
        .parent()
        .and_then(|m| m.child_by_field_name("object"))
        .filter(|o| o.kind() == "member_expression")
        .and_then(|o| o.child_by_field_name("property"))
        .is_some_and(|p| p.utf8_text(source_bytes) == Ok("prototype"))
}

pub(super) fn is_jsx_intrinsic_tag_call(
    name_node: Option<tree_sitter::Node<'_>>,
    name: &str,
) -> bool {
    let is_jsx_tag = name_node
        .and_then(|n| n.parent())
        .is_some_and(|p| matches!(p.kind(), "jsx_opening_element" | "jsx_self_closing_element"));
    is_jsx_tag && name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
}

fn resolve_workspace_package(clean: &str, file_index: &HashMap<&str, u32>) -> Option<u32> {
    let without_scope = clean.strip_prefix('@')?.split_once('/')?.1;
    let (pkg, subpath) = match without_scope.split_once('/') {
        Some((p, sub)) => (p, Some(sub)),
        None => (without_scope, None),
    };
    let short_pkg = pkg.split_once('-').map_or(pkg, |(_, rest)| rest);
    for cand_pkg in [pkg, short_pkg] {
        let base = match subpath {
            Some(sub) => format!("packages/{cand_pkg}/src/{sub}"),
            None => format!("packages/{cand_pkg}/src/index"),
        };
        if let Some(id) = probe_with_suffixes(&base, file_index, WEB_SUFFIXES) {
            return Some(id);
        }
    }
    None
}

pub(super) fn resolve_web_import_target(
    importer_path: &str,
    raw_target: &str,
    file_index: &HashMap<&str, u32>,
) -> Option<u32> {
    let clean = strip_import_quotes(raw_target);
    if clean.starts_with('.') {
        return resolve_relative_or_exact(importer_path, clean, file_index, WEB_SUFFIXES);
    }
    if clean.starts_with('@') {
        return resolve_workspace_package(clean, file_index);
    }
    None
}

pub(super) const TS_CONFIG_MANIFESTS: &[&str] = &["tsconfig.json", "jsconfig.json"];

/// `compilerOptions.paths` of a tsconfig/jsconfig as `(pattern, first target)`, targets joined with
/// `baseUrl`; the file may hold comments and trailing commas.
pub(super) fn parse_tsconfig_paths(content: &str) -> Vec<(String, String)> {
    let Ok(config) = serde_json::from_str::<serde_json::Value>(&strip_jsonc(content)) else {
        return Vec::new();
    };
    let options = &config["compilerOptions"];
    let base = options["baseUrl"].as_str().unwrap_or(".");
    let Some(paths) = options["paths"].as_object() else {
        return Vec::new();
    };
    paths
        .iter()
        .filter_map(|(pattern, targets)| {
            let target = targets.as_array()?.first()?.as_str()?;
            Some((pattern.clone(), super::normalize_lexical_path(base, target)))
        })
        .collect()
}

type CharStream<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// JSON text of a JSONC document: comments and trailing commas outside strings removed.
fn strip_jsonc(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('"', _) => copy_json_string(&mut chars, &mut out),
            ('/', Some('/' | '*')) => skip_jsonc_comment(&mut chars),
            ('}' | ']', _) => {
                let kept = out.trim_end().len();
                if out[..kept].ends_with(',') {
                    out.truncate(kept - 1);
                }
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Copy a string literal whose opening quote was just read, through its closing quote.
fn copy_json_string(chars: &mut CharStream<'_>, out: &mut String) {
    out.push('"');
    while let Some(c) = chars.next() {
        out.push(c);
        match c {
            '\\' => out.extend(chars.next()),
            '"' => return,
            _ => {}
        }
    }
}

/// Skip a `//` or `/* */` comment whose leading `/` was just read.
fn skip_jsonc_comment(chars: &mut CharStream<'_>) {
    if chars.next() == Some('/') {
        while chars.next_if(|&c| c != '\n').is_some() {}
        return;
    }
    while let Some(c) = chars.next() {
        if c == '*' && chars.next_if_eq(&'/').is_some() {
            return;
        }
    }
}

impl LanguageSemantics for JavaScriptLanguage {
    fn tag(&self) -> &'static str {
        "js"
    }

    fn is_generated_file_name(&self, file_name: &str) -> bool {
        file_name.ends_with(".min.js")
    }
    super::impl_grammar_method!(crate tree_sitter_javascript::LANGUAGE, "../queries/javascript/tags.scm");

    fn extensions(&self) -> &'static [&'static str] {
        &["js", "jsx", "mjs", "cjs"]
    }

    fn test_runner_manifest(&self) -> Option<&'static str> {
        Some("package.json")
    }

    fn test_run_command(&self, path: &str, source: &str, manifest: Option<&str>) -> Option<String> {
        super::js_test_run_command(path, source, manifest, true)
    }

    fn display_color(&self) -> &'static str {
        "#f1e05a"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Web
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        match ctx.sub {
            Some("constant") => super::is_screaming_snake(ctx.name),
            Some("cjsexport") => ctx
                .name_node
                .is_some_and(|nn| is_cjs_export_target(nn, ctx.source_bytes)),
            Some("protomethod") => ctx
                .name_node
                .is_some_and(|nn| is_prototype_member_target(nn, ctx.source_bytes)),
            _ => true,
        }
    }

    fn should_ignore_call(&self, name_node: Option<tree_sitter::Node<'_>>, name: &str) -> bool {
        is_jsx_intrinsic_tag_call(name_node, name)
    }

    fn std_member_names(&self) -> &'static [&'static str] {
        JS_STD_MEMBERS
    }

    fn std_qualifier_names(&self) -> &'static [&'static str] {
        JS_STD_QUALIFIERS
    }

    fn import_alias_manifests(&self) -> &'static [&'static str] {
        TS_CONFIG_MANIFESTS
    }

    fn parse_import_aliases(&self, content: &str) -> Vec<(String, String)> {
        parse_tsconfig_paths(content)
    }

    fn resolve_import_target(
        &self,
        importer_path: &str,
        raw_target: &str,
        file_index: &HashMap<&str, u32>,
    ) -> Option<u32> {
        resolve_web_import_target(importer_path, raw_target, file_index)
    }
}
