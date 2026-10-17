use super::{
    DialectGroup, LanguageSemantics, probe_with_suffixes, resolve_relative_or_exact,
    strip_import_quotes,
};
use crate::domain::Lang;
use crate::ingest::{FileParseResult, astro, parse_file_tree};
use std::collections::HashMap;
use tree_sitter::{Language, Query};

pub struct TypeScriptLanguage;
pub struct JavaScriptLanguage;
pub struct HtmlLanguage;
pub struct CssLanguage;

pub static TYPESCRIPT_LANG: TypeScriptLanguage = TypeScriptLanguage;
pub static JAVASCRIPT_LANG: JavaScriptLanguage = JavaScriptLanguage;
pub static HTML_LANG: HtmlLanguage = HtmlLanguage;
pub static CSS_LANG: CssLanguage = CssLanguage;

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

fn is_js_builtin_member_name(name: &str) -> bool {
    matches!(
        name,
        "push"
            | "pop"
            | "shift"
            | "unshift"
            | "slice"
            | "splice"
            | "map"
            | "filter"
            | "reduce"
            | "forEach"
            | "find"
            | "findIndex"
            | "includes"
            | "indexOf"
            | "join"
            | "split"
            | "trim"
            | "replace"
            | "replaceAll"
            | "startsWith"
            | "endsWith"
            | "toLowerCase"
            | "toUpperCase"
            | "toString"
            | "valueOf"
            | "keys"
            | "values"
            | "entries"
            | "then"
            | "catch"
            | "finally"
            | "test"
            | "match"
            | "has"
            | "get"
            | "set"
            | "delete"
            | "add"
            | "clear"
            | "some"
            | "every"
            | "flat"
            | "flatMap"
            | "sort"
            | "reverse"
            | "concat"
            | "fill"
            | "at"
            | "charAt"
            | "charCodeAt"
            | "codePointAt"
            | "padStart"
            | "padEnd"
            | "repeat"
            | "search"
            | "matchAll"
            | "exec"
            | "toFixed"
            | "toISOString"
            | "getTime"
            | "resolve"
            | "reject"
            | "all"
            | "allSettled"
            | "race"
            | "any"
            | "from"
            | "of"
            | "isArray"
            | "assign"
            | "freeze"
            | "create"
            | "defineProperty"
            | "hasOwn"
            | "hasOwnProperty"
            | "parse"
            | "stringify"
            | "now"
            | "Date"
            | "dirname"
            | "basename"
            | "extname"
            | "relative"
            | "isAbsolute"
    )
}

fn is_js_builtin_qualifier_name(name: &str) -> bool {
    matches!(
        name,
        "Array"
            | "Object"
            | "JSON"
            | "Math"
            | "Number"
            | "String"
            | "Boolean"
            | "Symbol"
            | "BigInt"
            | "Date"
            | "RegExp"
            | "Promise"
            | "Reflect"
            | "Proxy"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "Error"
            | "TypeError"
            | "RangeError"
            | "SyntaxError"
            | "Uint8Array"
            | "Int32Array"
            | "Float64Array"
            | "ArrayBuffer"
            | "DataView"
            | "Buffer"
            | "console"
            | "process"
            | "URL"
            | "URLSearchParams"
            | "Intl"
            | "crypto"
            | "TextEncoder"
            | "TextDecoder"
            | "Performance"
            | "performance"
            | "navigator"
            | "window"
            | "document"
            | "globalThis"
            | "fs"
            | "path"
            | "os"
            | "url"
            | "util"
            | "child_process"
            | "Bun"
            | "Deno"
    )
}

impl LanguageSemantics for TypeScriptLanguage {
    fn tag(&self) -> &'static str {
        "ts"
    }

    fn display_color(&self) -> &'static str {
        "#3178c6"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Web
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_js_builtin_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_js_builtin_qualifier_name(qualifier)
    }

    fn parse_file_override(
        &self,
        source_bytes: &[u8],
        rel_path: &str,
        language: &Language,
        query: &Query,
    ) -> Option<FileParseResult> {
        if !rel_path.ends_with(".astro") {
            return None;
        }
        Some(match astro::extract_astro_frontmatter(source_bytes) {
            astro::AstroFrontmatter::Ok {
                start_byte,
                end_byte,
                start_row,
            } => {
                let sub_bytes = &source_bytes[start_byte..end_byte];
                let mut res = parse_file_tree(sub_bytes, language, query, Lang::TypeScript);
                for d in &mut res.defs {
                    d.line += start_row;
                    d.start_byte += start_byte;
                    d.end_byte += start_byte;
                }
                for r in &mut res.refs {
                    r.line += start_row;
                    r.start_byte += start_byte;
                }
                res.line_count = source_bytes.iter().filter(|&&b| b == b'\n').count() as u32 + 1;
                res.byte_size = source_bytes.len() as u64;
                res
            }
            _ => FileParseResult::default(),
        })
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

impl LanguageSemantics for JavaScriptLanguage {
    fn tag(&self) -> &'static str {
        "js"
    }

    fn display_color(&self) -> &'static str {
        "#f1e05a"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Web
    }

    fn is_std_or_builtin_member(&self, name: &str) -> bool {
        is_js_builtin_member_name(name)
    }

    fn is_std_or_builtin_qualifier(&self, qualifier: &str) -> bool {
        is_js_builtin_qualifier_name(qualifier)
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

impl LanguageSemantics for HtmlLanguage {
    fn tag(&self) -> &'static str {
        "html"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Markup
    }

    fn supports_slice(&self) -> bool {
        false
    }
}

impl LanguageSemantics for CssLanguage {
    fn tag(&self) -> &'static str {
        "css"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Markup
    }

    fn supports_slice(&self) -> bool {
        false
    }
}

fn resolve_workspace_package(clean: &str, file_index: &HashMap<&str, u32>) -> Option<u32> {
    let without_scope = clean.strip_prefix('@')?.split_once('/')?.1;
    let (pkg, subpath) = match without_scope.split_once('/') {
        Some((p, sub)) => (p, Some(sub)),
        None => (without_scope, None),
    };
    let short_pkg = pkg.strip_prefix("pi-").unwrap_or(pkg);
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

pub(crate) fn resolve_web_import_target(
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
