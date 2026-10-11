pub mod astro;
pub mod markdown;
pub mod yaml_guard;

use crate::crawler::CrawledFile;
use crate::domain::{
    FileInfo, IngestResult, Interner, Lang, NO_NODE, NodeId, RefRole, Reference, SymKind, Symbol,
};
use rayon::prelude::*;
use std::cell::RefCell;
use std::fs;
use std::sync::{Arc, LazyLock};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

unsafe extern "C" {
    fn tree_sitter_elixir() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_dart() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_kotlin() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_ruby() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_php() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_lua() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_gdscript() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_json() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_toml() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_yaml() -> *const tree_sitter::ffi::TSLanguage;
    fn tree_sitter_markdown() -> *const tree_sitter::ffi::TSLanguage;
}

macro_rules! lang_entry {
    ($lang:expr, $query_src:expr) => {
        LazyLock::new(|| {
            let ts: Language = $lang;
            let q = Query::new(&ts, $query_src).expect("valid query");
            (ts, Arc::new(q))
        })
    };
}
thread_local! {
    static PARSER_STATE: RefCell<(Parser, QueryCursor)> = RefCell::new((
        Parser::new(),
        QueryCursor::new(),
    ));
}

static LANG_TABLE: [LazyLock<(Language, Arc<Query>)>; 24] = [
    lang_entry!(
        tree_sitter_rust::LANGUAGE.into(),
        include_str!("../queries/rust/tags.scm")
    ),
    lang_entry!(
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        include_str!("../queries/typescript/tags.scm")
    ),
    lang_entry!(
        tree_sitter_javascript::LANGUAGE.into(),
        include_str!("../queries/javascript/tags.scm")
    ),
    lang_entry!(
        tree_sitter_python::LANGUAGE.into(),
        include_str!("../queries/python/tags.scm")
    ),
    lang_entry!(
        tree_sitter_go::LANGUAGE.into(),
        include_str!("../queries/go/tags.scm")
    ),
    lang_entry!(
        tree_sitter_c::LANGUAGE.into(),
        include_str!("../queries/c/tags.scm")
    ),
    lang_entry!(
        tree_sitter_cpp::LANGUAGE.into(),
        include_str!("../queries/cpp/tags.scm")
    ),
    lang_entry!(
        tree_sitter_cuda::LANGUAGE.into(),
        include_str!("../queries/cuda/tags.scm")
    ),
    lang_entry!(
        tree_sitter_cpp::LANGUAGE.into(),
        include_str!("../queries/cpp/tags.scm")
    ),
    lang_entry!(
        tree_sitter_java::LANGUAGE.into(),
        include_str!("../queries/java/tags.scm")
    ),
    lang_entry!(
        tree_sitter_c_sharp::LANGUAGE.into(),
        include_str!("../queries/csharp/tags.scm")
    ),
    lang_entry!(
        tree_sitter_swift::LANGUAGE.into(),
        include_str!("../queries/swift/tags.scm")
    ),
    lang_entry!(
        tree_sitter_bash::LANGUAGE.into(),
        include_str!("../queries/bash/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_ruby()) },
        include_str!("../queries/ruby/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_php()) },
        include_str!("../queries/php/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_lua()) },
        include_str!("../queries/lua/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_elixir()) },
        include_str!("../queries/elixir/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_dart()) },
        include_str!("../queries/dart/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_kotlin()) },
        include_str!("../queries/kotlin/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_gdscript()) },
        include_str!("../queries/gdscript/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_json()) },
        include_str!("../queries/json/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_toml()) },
        include_str!("../queries/toml/tags.scm")
    ),
    lang_entry!(
        unsafe { Language::from_raw(tree_sitter_yaml()) },
        include_str!("../queries/yaml/tags.scm")
    ),
    lang_entry!(unsafe { Language::from_raw(tree_sitter_markdown()) }, ""),
];

/// Registry of supported Tree-Sitter languages and compiled queries.
#[derive(Default, Clone, Copy)]
pub struct LanguageRegistry;

impl LanguageRegistry {
    /// Initialize registry with all supported 24 languages.
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self)
    }

    /// Initialize registry with Tier-1 languages.
    pub fn new_tier1() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new()
    }

    /// Find compiled language and query for given Lang.
    pub fn get(&self, lang: Lang) -> Option<(&Language, &Query)> {
        let idx = lang as usize;
        if idx < LANG_TABLE.len() {
            let (ts, q) = &*LANG_TABLE[idx];
            Some((ts, q.as_ref()))
        } else {
            None
        }
    }
}

/// Raw definition extracted before deduplication and global ID assignment.
#[derive(Debug, Clone)]
pub struct RawDef {
    pub name: String,
    pub scope: Option<String>,
    pub kind: SymKind,
    pub line: u32,
    pub start_byte: usize,
    pub end_byte: usize,
    pub params: u16,
    pub metrics: crate::metrics::ComplexityMetrics,
}

/// Raw reference extracted before enclosing symbol attribution.
#[derive(Debug, Clone)]
pub struct RawRef {
    pub callee_name: String,
    pub qualifier: Option<String>,
    pub line: u32,
    pub start_byte: usize,
    pub role: RefRole,
    pub arg_count: u16,
    pub arg_count_known: bool,
}

/// Parsed facts from a single file.
#[derive(Debug, Default)]
pub struct FileParseResult {
    pub line_count: u32,
    pub byte_size: u64,
    pub defs: Vec<RawDef>,
    pub refs: Vec<RawRef>,
}

enum Capture {
    Name,
    Def(SymKind, Option<&'static str>),
    Ref(RefRole),
}

fn classify_capture(name: &str) -> Option<Capture> {
    if name == "name" {
        return Some(Capture::Name);
    }
    if let Some(tail) = name.strip_prefix("definition.") {
        let (kind, sub) = match tail {
            "function" => (SymKind::Function, None),
            "cjsexport" => (SymKind::Function, Some("cjsexport")),
            "method" => (SymKind::Method, None),
            "protomethod" => (SymKind::Method, Some("protomethod")),
            "class" | "object" => (SymKind::Class, None),
            "struct" => (SymKind::Struct, None),
            "interface" => (SymKind::Interface, None),
            "var" | "constant" | "attribute" | "property" => (SymKind::Var, None),
            "macro" => (SymKind::Macro, None),
            "module" | "section" | "yamlkey" => (SymKind::Section, None),
            "field" => (SymKind::Field, None),
            _ => (SymKind::Other, None),
        };
        return Some(Capture::Def(kind, sub));
    }
    name.strip_prefix("reference.").map(|tail| {
        Capture::Ref(match tail {
            "read" | "attribute" => RefRole::Read,
            "write" => RefRole::Write,
            "import" => RefRole::Import,
            "extends" | "implementation" => RefRole::Extends,
            "macro" => RefRole::Macro,
            "type" => RefRole::Type,
            "value" => RefRole::Value,
            "through" => RefRole::Through,
            _ => RefRole::Call,
        })
    })
}

fn extract_impl_scope(parent: tree_sitter::Node<'_>, source_bytes: &[u8]) -> Option<String> {
    let type_node = parent.child_by_field_name("type")?;
    let text = type_node.utf8_text(source_bytes).ok()?;
    let base = text.split('<').next().unwrap_or(text).trim();
    if base.is_empty() {
        None
    } else {
        Some(base.to_string())
    }
}

fn find_positional_type_ident(
    parent: tree_sitter::Node<'_>,
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> Option<String> {
    for i in 0..parent.named_child_count() {
        let child = parent.named_child(i)?;
        if child.kind() == "type_identifier" && child != name_node {
            let text = child.utf8_text(source_bytes).ok()?.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn extract_named_container_scope(
    parent: tree_sitter::Node<'_>,
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> Option<String> {
    if let Some(name_child) = parent.child_by_field_name("name") {
        if name_child == name_node {
            return None;
        }
        let text = name_child.utf8_text(source_bytes).ok()?.trim();
        return if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        };
    }
    find_positional_type_ident(parent, name_node, source_bytes)
}

fn extract_node_scope(
    parent: tree_sitter::Node<'_>,
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> Option<String> {
    match parent.kind() {
        "impl_item" => extract_impl_scope(parent, source_bytes),
        "mod_item" | "trait_item" | "class_declaration" | "class_definition"
        | "struct_specifier" | "object_declaration" | "companion_object" => {
            extract_named_container_scope(parent, name_node, source_bytes)
        }
        _ => None,
    }
}

fn find_enclosing_scope(name_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> Option<String> {
    let mut curr = name_node.parent();
    while let Some(parent) = curr {
        if let Some(scope) = extract_node_scope(parent, name_node, source_bytes) {
            return Some(scope);
        }
        curr = parent.parent();
    }
    None
}

fn find_call_qualifier(name_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> Option<String> {
    let parent = name_node.parent()?;
    if parent.kind() == "scoped_identifier"
        && let Some(path_node) = parent.child_by_field_name("path")
        && let Ok(text) = path_node.utf8_text(source_bytes)
    {
        let last = text.rsplit("::").next().unwrap_or(text);
        let base = last.split('<').next().unwrap_or(last).trim();
        if !base.is_empty() {
            return Some(base.to_string());
        }
    }
    None
}

fn count_arguments_in_list(child: tree_sitter::Node<'_>) -> u16 {
    let mut count = 0u16;
    for j in 0..child.named_child_count() {
        if let Some(arg) = child.named_child(j)
            && !arg.kind().contains("comment")
        {
            count = count.saturating_add(1);
        }
    }
    count
}

fn find_arguments_child(parent: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    for i in 0..parent.child_count() {
        if let Some(child) = parent.child(i) {
            let ckind = child.kind();
            if ckind.contains("argument") || ckind == "args" {
                return Some(child);
            }
        }
    }
    None
}

fn count_call_arguments(name_node: tree_sitter::Node<'_>) -> (u16, bool) {
    let mut curr = name_node.parent();
    while let Some(parent) = curr {
        let kind = parent.kind();
        if kind.contains("call") || kind == "invocation" || kind == "method_invocation" {
            if let Some(args_node) = find_arguments_child(parent) {
                return (count_arguments_in_list(args_node), true);
            }
            break;
        }
        curr = parent.parent();
    }
    (0, false)
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

fn elixir_arguments<'a>(node: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    (0..node.named_child_count())
        .filter_map(|i| node.named_child(i))
        .find(|c| c.kind() == "arguments")
}

fn elixir_first_arg<'a>(node: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    let args = elixir_arguments(node)?;
    for i in 0..args.named_child_count() {
        let child = args.named_child(i)?;
        if child.kind() != "comment" {
            return Some(child);
        }
    }
    None
}

fn unwrap_when_guard<'a>(
    head: tree_sitter::Node<'a>,
    source_bytes: &[u8],
) -> Option<tree_sitter::Node<'a>> {
    let op = head.child_by_field_name("operator")?;
    if op.utf8_text(source_bytes).ok() == Some("when") {
        head.child_by_field_name("left")
    } else {
        None
    }
}

fn count_non_comment_args(args: tree_sitter::Node<'_>) -> u16 {
    let mut count = 0u16;
    for i in 0..args.named_child_count() {
        if let Some(c) = args.named_child(i)
            && c.kind() != "comment"
        {
            count = count.saturating_add(1);
        }
    }
    count
}

fn elixir_param_count(def_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> u16 {
    let Some(mut head) = elixir_first_arg(def_node) else {
        return 0;
    };
    if head.kind() == "binary_operator" {
        match unwrap_when_guard(head, source_bytes) {
            Some(left) => head = left,
            None => return 2,
        }
    }
    if head.kind() == "unary_operator" {
        return 1;
    }
    elixir_arguments(head)
        .map(count_non_comment_args)
        .unwrap_or(0)
}
struct MatchExtract<'a> {
    name: Option<String>,
    name_node: Option<tree_sitter::Node<'a>>,
    def: Option<(SymKind, Option<&'static str>, usize, usize, u32)>,
    def_node: Option<tree_sitter::Node<'a>>,
    r#ref: Option<(RefRole, usize, u32)>,
}

fn extract_match_captures<'a>(
    m: &tree_sitter::QueryMatch<'a, '_>,
    query: &Query,
    source_bytes: &[u8],
) -> MatchExtract<'a> {
    let mut ext = MatchExtract {
        name: None,
        name_node: None,
        def: None,
        def_node: None,
        r#ref: None,
    };
    for capture in m.captures {
        let cap_name = query.capture_names()[capture.index as usize];
        let node = capture.node;
        match classify_capture(cap_name) {
            Some(Capture::Name) => {
                if let Ok(text) = node.utf8_text(source_bytes) {
                    ext.name = Some(text.to_string());
                    ext.name_node = Some(node);
                }
            }
            Some(Capture::Def(k, sub)) => {
                let line = node.start_position().row as u32 + 1;
                ext.def = Some((k, sub, node.start_byte(), node.end_byte(), line));
                ext.def_node = Some(node);
            }
            Some(Capture::Ref(r)) => {
                let line = node.start_position().row as u32 + 1;
                ext.r#ref = Some((r, node.start_byte(), line));
            }
            None => {}
        }
    }
    ext
}

fn is_cjs_export_target(name_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
    let Some(member) = name_node.parent() else {
        return false;
    };
    let Some(obj) = member.child_by_field_name("object") else {
        return false;
    };
    if obj.kind() == "identifier" {
        return obj.utf8_text(source_bytes) == Ok("exports");
    }
    if obj.kind() == "member_expression" {
        let Some(oo) = obj.child_by_field_name("object") else {
            return false;
        };
        let Some(prop) = obj.child_by_field_name("property") else {
            return false;
        };
        return oo.kind() == "identifier"
            && oo.utf8_text(source_bytes) == Ok("module")
            && prop.utf8_text(source_bytes) == Ok("exports");
    }
    false
}

fn is_prototype_member_target(name_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
    let Some(member) = name_node.parent() else {
        return false;
    };
    let Some(obj) = member.child_by_field_name("object") else {
        return false;
    };
    if obj.kind() != "member_expression" {
        return false;
    }
    let Some(prop) = obj.child_by_field_name("property") else {
        return false;
    };
    prop.utf8_text(source_bytes) == Ok("prototype")
}

fn is_gated_def_allowed(
    sub: Option<&str>,
    node: Option<tree_sitter::Node<'_>>,
    src: &[u8],
) -> bool {
    match sub {
        Some("cjsexport") => node.is_some_and(|nn| is_cjs_export_target(nn, src)),
        Some("protomethod") => node.is_some_and(|nn| is_prototype_member_target(nn, src)),
        _ => true,
    }
}

fn record_def_item(
    ext: &MatchExtract<'_>,
    source_bytes: &[u8],
    lang: Lang,
    defs: &mut Vec<RawDef>,
) {
    let (Some(orig_name), Some((kind, sub, start, mut end, line))) = (&ext.name, ext.def) else {
        return;
    };
    if !is_gated_def_allowed(sub, ext.name_node, source_bytes) {
        return;
    }
    let scope = ext
        .name_node
        .and_then(|n| find_enclosing_scope(n, source_bytes));

    let mut body_node = ext.def_node;
    if lang == Lang::Dart
        && let Some(dn) = ext.def_node
        && let Some(body) = dart_following_body(dn)
    {
        end = body.end_byte();
        body_node = Some(body);
    }

    let metrics = if kind == SymKind::Function || kind == SymKind::Method {
        body_node
            .map(|n| crate::metrics::compute_complexity_from_node(n, source_bytes, lang))
            .unwrap_or_default()
    } else {
        crate::metrics::ComplexityMetrics::default()
    };

    let params = if lang == Lang::Elixir {
        ext.def_node
            .map(|n| elixir_param_count(n, source_bytes))
            .unwrap_or(0)
    } else {
        ext.def_node
            .map(crate::gate::contract::extract_param_count)
            .unwrap_or(0)
    };
    let name = if lang == Lang::Elixir && kind == SymKind::Function {
        format!("{orig_name}/{params}")
    } else {
        orig_name.clone()
    };

    defs.push(RawDef {
        name,
        scope,
        kind,
        line,
        start_byte: start,
        end_byte: end,
        params,
        metrics,
    });
}

fn record_ref_item(ext: &MatchExtract<'_>, source_bytes: &[u8], refs: &mut Vec<RawRef>) {
    let (Some(name), Some((role, start, line))) = (&ext.name, ext.r#ref) else {
        return;
    };

    let qualifier = ext
        .name_node
        .and_then(|n| find_call_qualifier(n, source_bytes));
    let (arg_count, arg_count_known) = ext
        .name_node
        .map(count_call_arguments)
        .unwrap_or((0, false));

    refs.push(RawRef {
        callee_name: name.clone(),
        qualifier,
        line,
        start_byte: start,
        role,
        arg_count,
        arg_count_known,
    });
}

fn record_extracted_item(
    ext: MatchExtract<'_>,
    source_bytes: &[u8],
    lang: Lang,
    defs: &mut Vec<RawDef>,
    refs: &mut Vec<RawRef>,
) {
    if ext.def.is_some() {
        record_def_item(&ext, source_bytes, lang, defs);
    } else if ext.r#ref.is_some() {
        record_ref_item(&ext, source_bytes, refs);
    }
}

fn parse_file_tree(
    source_bytes: &[u8],
    language: &Language,
    query: &Query,
    lang: Lang,
) -> FileParseResult {
    PARSER_STATE.with(|state| {
        let mut borrowed = state.borrow_mut();
        let (parser, cursor) = &mut *borrowed;
        if parser.set_language(language).is_err() {
            return FileParseResult::default();
        }

        let tree = match parser.parse(source_bytes, None) {
            Some(t) => t,
            None => return FileParseResult::default(),
        };

        let line_count = source_bytes.iter().filter(|&&b| b == b'\n').count() as u32 + 1;
        let mut defs = Vec::new();
        let mut refs = Vec::new();

        {
            let mut matches = cursor.matches(query, tree.root_node(), source_bytes);
            while let Some(m) = matches.next() {
                let ext = extract_match_captures(m, query, source_bytes);
                record_extracted_item(ext, source_bytes, lang, &mut defs, &mut refs);
            }
        }

        defs.sort_by(|a, b| {
            a.start_byte
                .cmp(&b.start_byte)
                .then_with(|| a.name.cmp(&b.name))
        });
        defs.dedup_by(|a, b| a.start_byte == b.start_byte && a.name == b.name);

        FileParseResult {
            line_count,
            byte_size: source_bytes.len() as u64,
            defs,
            refs,
        }
    })
}

/// Parse a single file content with a compiled grammar and query.
pub fn parse_file_content(
    source_bytes: &[u8],
    language: &Language,
    query: &Query,
    lang: Lang,
    rel_path: &str,
) -> FileParseResult {
    if lang == Lang::Markdown {
        return markdown::extract_markdown(source_bytes, rel_path, language);
    }

    if lang == Lang::Yaml && yaml_guard::yaml_nests_too_deep(source_bytes) {
        return FileParseResult::default();
    }

    if rel_path.ends_with(".astro") {
        return match astro::extract_astro_frontmatter(source_bytes) {
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
        };
    }

    parse_file_tree(source_bytes, language, query, lang)
}

/// Ingest crawled files in parallel and assemble IngestResult.
pub fn ingest_crawled_files(
    files: &[CrawledFile],
    registry: &LanguageRegistry,
    interner: &Interner,
) -> IngestResult {
    let parsed_files: Vec<Option<FileParseResult>> = files
        .par_iter()
        .map(|file| {
            let (language, query) = registry.get(file.lang)?;
            let source_bytes = fs::read(&file.path).ok()?;
            if crate::crawler::is_binary_buffer(&source_bytes) {
                return None;
            }
            Some(parse_file_content(
                &source_bytes,
                language,
                query,
                file.lang,
                &file.rel_path,
            ))
        })
        .collect();

    let mut ingest = IngestResult::new();
    let mut next_node_id: NodeId = 0;

    for (file_id, (file, parsed)) in files.iter().zip(parsed_files).enumerate() {
        let file_id = file_id as u32;
        let file_spur = interner.intern(&file.rel_path);
        let (line_count, byte_size, defs, refs) = match parsed {
            Some(p) => (p.line_count, p.byte_size, p.defs, p.refs),
            None => (0, file.byte_size, Vec::new(), Vec::new()),
        };

        ingest.files.push(FileInfo {
            id: file_id,
            path: file_spur,
            lang: file.lang,
            line_count,
            byte_size,
        });

        let file_symbols = build_file_symbols(defs, file, file_id, interner, &mut next_node_id);
        let file_refs = build_file_references(refs, file, file_id, &file_symbols, interner);

        ingest.references.extend(file_refs);
        ingest.symbols.extend(file_symbols);
    }
    ingest
}

/// Ingest in-memory source files directly without filesystem I/O.
pub fn ingest_in_memory_files(
    files: &[(&str, Lang, &[u8])],
    registry: &LanguageRegistry,
    interner: &Interner,
) -> IngestResult {
    let mut ingest = IngestResult::new();
    let mut next_node_id: NodeId = 0;

    for (file_id, &(rel_path, lang, source_bytes)) in files.iter().enumerate() {
        let file_id = file_id as u32;
        let file_spur = interner.intern(rel_path);

        let parsed = registry.get(lang).map(|(language, query)| {
            parse_file_content(source_bytes, language, query, lang, rel_path)
        });

        let (line_count, byte_size, defs, refs) = match parsed {
            Some(p) => (p.line_count, p.byte_size, p.defs, p.refs),
            None => (0, source_bytes.len() as u64, Vec::new(), Vec::new()),
        };

        ingest.files.push(FileInfo {
            id: file_id,
            path: file_spur,
            lang,
            line_count,
            byte_size,
        });

        let dummy_crawled = CrawledFile {
            path: std::path::PathBuf::from(rel_path),
            rel_path: rel_path.to_string(),
            lang,
            byte_size: source_bytes.len() as u64,
        };

        let file_symbols =
            build_file_symbols(defs, &dummy_crawled, file_id, interner, &mut next_node_id);
        let file_refs =
            build_file_references(refs, &dummy_crawled, file_id, &file_symbols, interner);

        ingest.references.extend(file_refs);
        ingest.symbols.extend(file_symbols);
    }

    ingest
}

fn build_file_symbols(
    defs: Vec<RawDef>,
    file: &CrawledFile,
    file_id: u32,
    interner: &Interner,
    next_node_id: &mut NodeId,
) -> Vec<Symbol> {
    let mut file_symbols = Vec::with_capacity(defs.len());
    for def in defs {
        let id = *next_node_id;
        *next_node_id += 1;
        let name = interner.intern(&def.name);
        let canon = match &def.scope {
            Some(sc) => interner.intern(&format!("{}::{}::{}", file.rel_path, sc, def.name)),
            None => interner.intern(&format!("{}::{}", file.rel_path, def.name)),
        };
        let mut sym = Symbol::new(id, name, canon, def.kind, file.lang, file_id, def.line);
        if let Some(sc) = &def.scope {
            sym.scope = Some(interner.intern(sc));
        }
        sym.sig_start_byte = def.start_byte as u32;
        sym.end_byte = def.end_byte as u32;
        sym.cx = def.metrics.cx;
        sym.ccx = def.metrics.ccx;
        sym.max_nest = def.metrics.max_nest;
        sym.params = def.params;
        sym.humps = def.metrics.humps;
        sym.deep = def.metrics.deep;
        sym.ev = def.metrics.ev;
        file_symbols.push(sym);
    }
    file_symbols
}

fn find_enclosing_symbol(file_symbols: &[Symbol], start_byte: usize) -> NodeId {
    let upper = file_symbols.partition_point(|s| (s.sig_start_byte as usize) <= start_byte);
    let mut best_id = NO_NODE;
    let mut best_span = u32::MAX;

    for s in file_symbols[..upper].iter().rev() {
        let sig_start = s.sig_start_byte as usize;
        let dist = start_byte - sig_start;
        if dist as u32 >= best_span {
            break;
        }
        if start_byte <= (s.end_byte as usize) {
            let span = s.end_byte - s.sig_start_byte;
            if span < best_span {
                best_span = span;
                best_id = s.id;
            }
        }
    }
    best_id
}

fn build_file_references(
    refs: Vec<RawRef>,
    file: &CrawledFile,
    file_id: u32,
    file_symbols: &[Symbol],
    interner: &Interner,
) -> Vec<Reference> {
    refs.into_iter()
        .map(|r| {
            let from_symbol = find_enclosing_symbol(file_symbols, r.start_byte);

            let callee_spur = interner.intern(&r.callee_name);
            let mut reference =
                Reference::new(from_symbol, callee_spur, file_id, r.line, file.lang, r.role);
            if let Some(q) = &r.qualifier {
                reference.qualifier = Some(interner.intern(q));
            }
            reference.start_byte = r.start_byte as u32;
            reference.arg_count = r.arg_count;
            reference.arg_count_known = r.arg_count_known;
            reference
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier1_registry_and_parsing() {
        let reg = LanguageRegistry::new_tier1().unwrap();
        assert!(reg.get(Lang::Rust).is_some() && reg.get(Lang::Python).is_some());

        let (lang, query) = reg.get(Lang::Rust).unwrap();
        let res = parse_file_content(
            b"fn run() { log(); } fn log() {}",
            lang,
            query,
            Lang::Rust,
            "src/lib.rs",
        );
        assert_eq!(res.defs.len(), 2);
        assert_eq!(res.refs.len(), 1);
        assert_eq!(res.refs[0].callee_name, "log");
    }

    #[test]
    fn test_classify_captures() {
        assert!(matches!(classify_capture("name"), Some(Capture::Name)));
        assert!(classify_capture("other").is_none());
        for tail in [
            "function",
            "cjsexport",
            "method",
            "protomethod",
            "class",
            "object",
            "struct",
            "interface",
            "var",
            "constant",
            "attribute",
            "property",
            "macro",
            "module",
            "section",
            "yamlkey",
            "field",
            "x",
        ] {
            assert!(classify_capture(&format!("definition.{tail}")).is_some());
        }
        for tail in [
            "read",
            "attribute",
            "write",
            "import",
            "extends",
            "implementation",
            "macro",
            "type",
            "value",
            "through",
            "call",
            "y",
        ] {
            assert!(classify_capture(&format!("reference.{tail}")).is_some());
        }
    }

    #[test]
    fn test_all_24_grammars() {
        let reg = LanguageRegistry::new().unwrap();
        let fixtures: Vec<(&str, Lang, &str, usize, usize)> = vec![
            ("src/main.rs", Lang::Rust, "fn main() { run(); }", 1, 1),
            (
                "src/index.ts",
                Lang::TypeScript,
                "function greet(name: string) { log(name); }",
                1,
                1,
            ),
            (
                "src/app.js",
                Lang::JavaScript,
                "function add(a, b) { return sum(a, b); }",
                1,
                1,
            ),
            (
                "src/worker.py",
                Lang::Python,
                "def process(x):\n    handle(x)",
                1,
                1,
            ),
            (
                "src/server.go",
                Lang::Go,
                "package main\nfunc Worker() { doWork() }",
                1,
                1,
            ),
            ("src/util.c", Lang::C, "void init(void) { setup(); }", 1, 1),
            (
                "src/engine.cpp",
                Lang::Cpp,
                "void start() { boot(); }",
                1,
                1,
            ),
            (
                "src/kernel.cu",
                Lang::Cuda,
                "__global__ void kernel() { launch(); }",
                1,
                1,
            ),
            (
                "src/shader.metal",
                Lang::Metal,
                "void compute_shader() { do_compute(); }",
                1,
                1,
            ),
            (
                "src/App.java",
                Lang::Java,
                "class App { void main() { start(); } }",
                2,
                1,
            ),
            (
                "src/Service.cs",
                Lang::CSharp,
                "class Service { void Run() { Execute(); } }",
                2,
                1,
            ),
            (
                "src/entry.swift",
                Lang::Swift,
                "func launch() { begin() }",
                1,
                1,
            ),
            ("src/script.sh", Lang::Bash, "run() { echo hi; }", 1, 0),
            (
                "src/item.rb",
                Lang::Ruby,
                "class Item\n  def show\n    render()\n  end\nend",
                2,
                1,
            ),
            (
                "src/ctrl.php",
                Lang::Php,
                "<?php\nclass Controller {\n  public function handle() {\n    execute();\n  }\n}",
                2,
                1,
            ),
            (
                "src/alg.lua",
                Lang::Lua,
                "function solve(x)\n  compute(x)\nend",
                1,
                1,
            ),
            (
                "src/calc.ex",
                Lang::Elixir,
                "defmodule Calc do\n  def add(a, b) do\n    sum(a, b)\n  end\nend",
                2,
                1,
            ),
            (
                "src/box.dart",
                Lang::Dart,
                "class Box {\n  int compute(int x) {\n    return transform(x);\n  }\n}",
                2,
                1,
            ),
            (
                "src/handler.kt",
                Lang::Kotlin,
                "class Handler {\n  fun process(id: Int, flag: Boolean) {\n    execute(id)\n  }\n}",
                2,
                1,
            ),
            (
                "src/hero.gd",
                Lang::GDScript,
                "class_name Hero\nfunc move(dir):\n  step(dir)",
                2,
                1,
            ),
            (
                "package.json",
                Lang::Json,
                "{\n  \"name\": \"ripwire\",\n  \"scripts\": { \"test\": \"cargo test\" }\n}",
                2,
                0,
            ),
            (
                "Cargo.toml",
                Lang::Toml,
                "[package]\nname = \"ripwire\"\nversion = \"0.1.0\"",
                2,
                0,
            ),
            (
                "config.yaml",
                Lang::Yaml,
                "name: pipeline\nstages:\n  build: true",
                2,
                0,
            ),
            (
                "README.md",
                Lang::Markdown,
                "# Project\nOverview\n## Architecture\nDetails",
                3,
                0,
            ),
        ];

        for (path, lang, code, min_defs, min_refs) in fixtures {
            let (ts_lang, query) = reg.get(lang).unwrap();
            let res = parse_file_content(code.as_bytes(), ts_lang, query, lang, path);
            assert!(
                res.defs.len() >= min_defs,
                "Language {:?} at {} expected at least {} defs, got {}",
                lang,
                path,
                min_defs,
                res.defs.len()
            );
            assert!(
                res.refs.len() >= min_refs,
                "Language {:?} at {} expected at least {} refs, got {}",
                lang,
                path,
                min_refs,
                res.refs.len()
            );
        }
    }

    #[test]
    fn test_specialized_language_handlers() {
        let reg = LanguageRegistry::new().unwrap();

        // Astro frontmatter extraction
        let astro_code =
            b"---\nimport Nav from './Nav.astro';\nfunction setup() { run(); }\n---\n<Nav />";
        let (ts_lang, query) = reg.get(Lang::TypeScript).unwrap();
        let astro_res = parse_file_content(
            astro_code,
            ts_lang,
            query,
            Lang::TypeScript,
            "src/Component.astro",
        );
        assert!(
            !astro_res.defs.is_empty(),
            "Astro should extract frontmatter defs"
        );
        assert_eq!(astro_res.defs[0].name, "setup");
        assert!(
            !astro_res.refs.is_empty(),
            "Astro should extract frontmatter refs"
        );
        assert_eq!(astro_res.refs[0].callee_name, "run");

        // Dart sibling function_body span extension
        let dart_code =
            b"class Calculator {\n  int add(int a, int b) {\n    return helper(a, b);\n  }\n}";
        let (dart_lang, dart_query) = reg.get(Lang::Dart).unwrap();
        let dart_res =
            parse_file_content(dart_code, dart_lang, dart_query, Lang::Dart, "calc.dart");
        let add_def = dart_res
            .defs
            .iter()
            .find(|d| d.name == "add")
            .expect("found add");
        assert!(
            add_def.end_byte > add_def.start_byte + 25,
            "Dart def span must extend through body"
        );

        // Elixir arity-qualified naming
        let elixir_code = b"defmodule M do\n  def sum(a, b) do\n    a + b\n  end\nend";
        let (ex_lang, ex_query) = reg.get(Lang::Elixir).unwrap();
        let ex_res = parse_file_content(elixir_code, ex_lang, ex_query, Lang::Elixir, "m.ex");
        assert!(
            ex_res.defs.iter().any(|d| d.name == "sum/2"),
            "Elixir function must be arity-qualified: sum/2"
        );

        // Kotlin positional scope and parameter count
        let kt_code =
            b"class Service {\n  fun handle(id: Int, flag: Boolean) {\n    run(id)\n  }\n}";
        let (kt_lang, kt_query) = reg.get(Lang::Kotlin).unwrap();
        let kt_res = parse_file_content(kt_code, kt_lang, kt_query, Lang::Kotlin, "Service.kt");
        let fn_def = kt_res
            .defs
            .iter()
            .find(|d| d.name == "handle")
            .expect("found handle");
        assert_eq!(fn_def.scope.as_deref(), Some("Service"));
        assert_eq!(fn_def.params, 2);
    }
}
