use super::registry::PARSER_STATE;
use crate::domain::{Lang, RefRole, SymKind};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Query};

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

pub enum Capture {
    Name,
    Def(SymKind, Option<&'static str>),
    Ref(RefRole),
}

pub fn classify_capture(name: &str) -> Option<Capture> {
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

pub fn find_positional_type_ident(
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

pub fn elixir_arguments<'a>(node: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
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

pub fn parse_file_tree(
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
