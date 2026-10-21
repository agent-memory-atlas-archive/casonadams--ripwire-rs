use super::javascript::{
    JS_STD_MEMBERS, JS_STD_QUALIFIERS, TS_CONFIG_MANIFESTS, is_jsx_intrinsic_tag_call,
    parse_tsconfig_paths, resolve_web_import_target,
};
use super::{DefCaptureGateContext, DialectGroup, LanguageSemantics};
use crate::domain::Lang;
use crate::ingest::{FileParseResult, astro, parse_file_tree};
use std::collections::HashMap;
use std::sync::LazyLock;
use tree_sitter::{Language, Query};

pub struct TypeScriptLanguage;
pub static TYPESCRIPT_LANG: TypeScriptLanguage = TypeScriptLanguage;

static TSX_LANG_AND_QUERY: LazyLock<(Language, Query)> = LazyLock::new(|| {
    let ts: Language = tree_sitter_typescript::LANGUAGE_TSX.into();
    let src = concat!(
        include_str!("../queries/typescript/tags.scm"),
        "\n(jsx_self_closing_element name: (identifier) @name) @reference.call\n",
        "(jsx_self_closing_element name: (member_expression property: (property_identifier) @name)) @reference.call\n",
        "(jsx_opening_element name: (identifier) @name) @reference.call\n",
        "(jsx_opening_element name: (member_expression property: (property_identifier) @name)) @reference.call\n"
    );
    let q = Query::new(&ts, src).expect("valid tsx query");
    (ts, q)
});

fn parse_astro_file(source_bytes: &[u8], language: &Language, query: &Query) -> FileParseResult {
    let astro::AstroFrontmatter::Ok {
        start_byte,
        end_byte,
        start_row,
    } = astro::extract_astro_frontmatter(source_bytes)
    else {
        return FileParseResult::default();
    };
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

impl LanguageSemantics for TypeScriptLanguage {
    fn tag(&self) -> &'static str {
        "ts"
    }
    super::impl_grammar_method!(crate tree_sitter_typescript::LANGUAGE_TYPESCRIPT, "../queries/typescript/tags.scm");

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx", "mts", "cts", "astro"]
    }

    fn test_runner_manifest(&self) -> Option<&'static str> {
        Some("package.json")
    }

    fn test_run_command(&self, path: &str, source: &str, manifest: Option<&str>) -> Option<String> {
        super::js_test_run_command(path, source, manifest, false)
    }

    fn display_color(&self) -> &'static str {
        "#3178c6"
    }

    fn dialect_group(&self) -> DialectGroup {
        DialectGroup::Web
    }

    fn is_def_capture_allowed(&self, ctx: &DefCaptureGateContext<'_>) -> bool {
        ctx.sub != Some("constant") || super::is_screaming_snake(ctx.name)
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

    fn parse_file_override(
        &self,
        source_bytes: &[u8],
        rel_path: &str,
        language: &Language,
        query: &Query,
    ) -> Option<FileParseResult> {
        if rel_path.ends_with(".tsx") {
            let (tsx_lang, tsx_query) = &*TSX_LANG_AND_QUERY;
            return Some(parse_file_tree(
                source_bytes,
                tsx_lang,
                tsx_query,
                Lang::TypeScript,
            ));
        }
        if !rel_path.ends_with(".astro") {
            return None;
        }
        Some(parse_astro_file(source_bytes, language, query))
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
