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
    pub body_byte: usize,
    pub end_byte: usize,
    pub loc: u32,
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
    /// Language the reference is written in when it differs from its file's (embedded SQL).
    pub lang: Option<crate::domain::Lang>,
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
    ImportPath,
    Def(SymKind, Option<&'static str>),
    Ref(RefRole),
}

pub fn classify_capture(name: &str) -> Option<Capture> {
    if name == "name" {
        return Some(Capture::Name);
    }
    if name == "import.path" {
        return Some(Capture::ImportPath);
    }
    if let Some(tail) = name.strip_prefix("definition.") {
        let (kind, sub) = match tail {
            "function" => (SymKind::Function, None),
            "cjsexport" => (SymKind::Function, Some("cjsexport")),
            "method" => (SymKind::Method, None),
            "protomethod" => (SymKind::Method, Some("protomethod")),
            "class" | "object" => (SymKind::Class, None),
            "specialization" => (SymKind::Class, Some("specialization")),
            "struct" => (SymKind::Struct, None),
            "interface" => (SymKind::Interface, None),
            "constant" => (SymKind::Var, Some("constant")),
            "enummember" => (SymKind::Var, Some("enummember")),
            "attribute" => (SymKind::Var, Some("attribute")),
            "type" | "var" | "property" => (SymKind::Var, None),
            "macro" => (SymKind::Macro, None),
            "testmacroblock" => (SymKind::Macro, Some("testmacroblock")),
            "yamlkey" => (SymKind::Section, Some("yamlkey")),
            "module" | "section" => (SymKind::Section, None),
            "field" => (SymKind::Field, None),
            _ => return None,
        };
        return Some(Capture::Def(kind, sub));
    }
    name.strip_prefix("reference.").and_then(|tail| {
        Some(Capture::Ref(match tail {
            "read" | "attribute" => RefRole::Read,
            "write" => RefRole::Write,
            "import" => RefRole::Import,
            "extends" | "implementation" | "class" => RefRole::Extends,
            "macro" => RefRole::Macro,
            "type" => RefRole::Type,
            "value" => RefRole::Value,
            "through" => RefRole::Through,
            "call" | "bare" | "operator" => RefRole::Call,
            _ => return None,
        }))
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

pub fn distinct_scope_tail(
    cand: tree_sitter::Node<'_>,
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
    sep: &str,
) -> Option<String> {
    if cand == name_node {
        return None;
    }
    let text = cand.utf8_text(source_bytes).ok()?.trim();
    let tail = if sep.is_empty() {
        text
    } else {
        text.rsplit(sep).next().unwrap_or(text)
    };
    let clean = tail.split('<').next().unwrap_or(tail).trim();
    (!clean.is_empty()).then(|| clean.to_string())
}

pub fn find_positional_type_ident(
    parent: tree_sitter::Node<'_>,
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> Option<String> {
    let first = (0..parent.named_child_count())
        .filter_map(|i| parent.named_child(i))
        .find(|c| matches!(c.kind(), "type_identifier" | "identifier"))?;
    distinct_scope_tail(first, name_node, source_bytes, "")
}

fn extract_named_container_scope(
    parent: tree_sitter::Node<'_>,
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> Option<String> {
    if let Some(name_child) = parent.child_by_field_name("name") {
        return distinct_scope_tail(name_child, name_node, source_bytes, "::");
    }
    find_positional_type_ident(parent, name_node, source_bytes)
}

fn extract_qualified_scope(
    parent: tree_sitter::Node<'_>,
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
) -> Option<String> {
    let scope_node = parent.child_by_field_name("scope")?;
    distinct_scope_tail(scope_node, name_node, source_bytes, "::")
}

fn is_named_container_kind(kind: &str) -> bool {
    matches!(
        kind,
        "mod_item"
            | "trait_item"
            | "trait_definition"
            | "trait_declaration"
            | "class"
            | "class_declaration"
            | "abstract_class_declaration"
            | "class_definition"
            | "class_specifier"
            | "class_interface"
            | "class_implementation"
            | "struct_specifier"
            | "struct_declaration"
            | "record_declaration"
            | "enum_declaration"
            | "interface_declaration"
            | "protocol_declaration"
            | "contract_declaration"
            | "library_declaration"
            | "namespace_definition"
            | "module"
            | "object_declaration"
            | "object_definition"
            | "companion_object"
            | "mixin_declaration"
            | "extension_declaration"
    )
}

fn extract_node_scope(
    parent: tree_sitter::Node<'_>,
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
    lang: Lang,
) -> Option<String> {
    if let Some(custom) = lang
        .semantics()
        .extract_custom_scope(parent, name_node, source_bytes)
    {
        return Some(custom);
    }
    match parent.kind() {
        "impl_item" => extract_impl_scope(parent, source_bytes),
        "qualified_identifier" => extract_qualified_scope(parent, name_node, source_bytes),
        k if is_named_container_kind(k) => {
            extract_named_container_scope(parent, name_node, source_bytes)
        }
        _ => None,
    }
}

fn find_enclosing_scope(
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
    lang: Lang,
) -> Option<String> {
    let mut curr = name_node.parent();
    while let Some(parent) = curr {
        if let Some(scope) = extract_node_scope(parent, name_node, source_bytes, lang) {
            return Some(scope);
        }
        curr = parent.parent();
    }
    None
}

fn is_member_receiver_parent(kind: &str) -> bool {
    matches!(
        kind,
        "field_expression"
            | "member_expression"
            | "member_access_expression"
            | "attribute"
            | "navigation_expression"
            | "navigation_suffix"
            | "method_invocation"
            | "selector_expression"
            | "call"
            | "dot"
            | "dot_index_expression"
            | "method_index_expression"
            | "unconditional_assignable_selector"
            | "conditional_assignable_selector"
            | "remote"
    )
}

fn find_parent_sibling_receiver(parent: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    match parent.kind() {
        "unconditional_assignable_selector" | "conditional_assignable_selector" => {
            parent.parent().and_then(|sel| sel.prev_named_sibling())
        }
        "navigation_suffix" => parent.parent().and_then(|nav| nav.named_child(0)),
        "remote" => parent
            .child_by_field_name("module")
            .and_then(|m| m.child_by_field_name("module")),
        _ => None,
    }
}

fn find_member_receiver_node(parent: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    if let Some(node) = find_parent_sibling_receiver(parent) {
        return Some(node);
    }
    for field in [
        "argument",
        "object",
        "value",
        "operand",
        "receiver",
        "table",
        "left",
        "expression",
    ] {
        if let Some(n) = parent.child_by_field_name(field) {
            return Some(n);
        }
    }
    None
}

fn is_pascal_type_ident(text: &str) -> bool {
    let is_screaming_const =
        text.len() >= 2 && text.contains('_') && !text.bytes().any(|b| b.is_ascii_lowercase());
    text.bytes().next().is_some_and(|b| b.is_ascii_uppercase())
        && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !is_screaming_const
}

fn extract_member_receiver_qualifier(
    parent: tree_sitter::Node<'_>,
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
    lang: Lang,
) -> Option<String> {
    if !is_member_receiver_parent(parent.kind()) {
        return None;
    }
    let recv = find_member_receiver_node(parent)?;
    if recv == name_node {
        return None;
    }
    let text = recv.utf8_text(source_bytes).ok()?.trim();
    if matches!(text, "self" | "this" | "Self") {
        return find_enclosing_scope(name_node, source_bytes, lang);
    }
    let keep = is_pascal_type_ident(text) || lang.semantics().is_std_or_builtin_qualifier(text);
    keep.then(|| text.to_string())
}

fn find_call_qualifier(
    name_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
    lang: Lang,
) -> Option<String> {
    let parent = name_node.parent()?;
    if matches!(parent.kind(), "scoped_identifier" | "qualified_identifier")
        && let Some(path_node) = parent
            .child_by_field_name("path")
            .or_else(|| parent.child_by_field_name("scope"))
        && let Ok(text) = path_node.utf8_text(source_bytes)
    {
        let last = text.rsplit("::").next().unwrap_or(text);
        let base = last.split('<').next().unwrap_or(last).trim();
        if !base.is_empty() {
            return Some(base.to_string());
        }
    }
    extract_member_receiver_qualifier(parent, name_node, source_bytes, lang)
}

fn split_top_level_qualified(raw: &str) -> (&str, Option<String>) {
    let bytes = raw.as_bytes();
    let mut angle_depth = 0u32;
    let mut last_sep = None;
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        match bytes[i] {
            b'<' => angle_depth += 1,
            b'>' => angle_depth = angle_depth.saturating_sub(1),
            b':' if bytes[i + 1] == b':' && angle_depth == 0 => {
                last_sep = Some(i);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    let (qual, tail) = match last_sep {
        Some(pos) => {
            let prefix = &raw[..pos];
            let last_q = prefix.rsplit("::").next().unwrap_or(prefix);
            let clean_q = last_q.split('<').next().unwrap_or(last_q).trim();
            let q_opt = (!clean_q.is_empty()).then(|| clean_q.to_string());
            (q_opt, &raw[pos + 2..])
        }
        None => (None, raw),
    };
    let bare = if tail.starts_with("operator") || tail.starts_with('<') {
        tail.trim()
    } else {
        tail.split('<').next().unwrap_or(tail).trim()
    };
    (bare, qual)
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

struct MatchExtract<'a> {
    name: Option<String>,
    name_node: Option<tree_sitter::Node<'a>>,
    def: Option<(SymKind, Option<&'static str>, usize, usize, u32)>,
    def_node: Option<tree_sitter::Node<'a>>,
    r#ref: Option<(RefRole, usize, u32)>,
}

fn apply_capture<'a>(
    ext: &mut MatchExtract<'a>,
    cap_name: &str,
    node: tree_sitter::Node<'a>,
    source_bytes: &[u8],
) {
    match classify_capture(cap_name) {
        Some(Capture::Name) => {
            if let Ok(text) = node.utf8_text(source_bytes) {
                ext.name = Some(text.to_string());
                ext.name_node = Some(node);
            }
        }
        Some(Capture::ImportPath) => {
            let line = node.start_position().row as u32 + 1;
            ext.r#ref = Some((RefRole::Import, node.start_byte(), line));
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
        apply_capture(&mut ext, cap_name, capture.node, source_bytes);
    }
    ext
}

fn is_body_block_kind(kind: &str) -> bool {
    matches!(
        kind,
        "compound_statement"
            | "function_body"
            | "block"
            | "statement_block"
            | "declaration_list"
            | "field_declaration_list"
            | "class_body"
            | "enum_class_body"
            | "do_block"
            | "body_statement"
            | "clause_body"
            | "template_body"
            | "indented_block"
            | "implementation_definition"
            | "protocol_body"
    )
}

fn is_callable_wrapper_kind(kind: &str) -> bool {
    matches!(
        kind,
        "lexical_declaration"
            | "variable_declaration"
            | "variable_declarator"
            | "public_field_definition"
            | "field_definition"
            | "property_declaration"
            | "export_statement"
            | "arrow_function"
            | "function_expression"
            | "function"
            | "function_definition"
            | "generator_function"
            | "as_expression"
            | "satisfies_expression"
            | "parenthesized_expression"
            | "assignment_statement"
            | "expression_list"
            | "table_constructor"
            | "field"
    )
}

fn child_body_start(ch: tree_sitter::Node<'_>) -> Option<usize> {
    if is_body_block_kind(ch.kind()) {
        return Some(ch.start_byte());
    }
    if is_callable_wrapper_kind(ch.kind()) {
        let nested = find_def_body_start(ch);
        return (nested > 0).then_some(nested);
    }
    None
}

fn find_def_body_start(node: tree_sitter::Node<'_>) -> usize {
    if is_body_block_kind(node.kind()) {
        return node.start_byte();
    }
    if let Some(b) = node.child_by_field_name("body") {
        return b.start_byte();
    }
    (0..node.child_count())
        .filter_map(|i| node.child(i).and_then(child_body_start))
        .next()
        .unwrap_or(0)
}

fn record_def_item(
    ext: &MatchExtract<'_>,
    source_bytes: &[u8],
    lang: Lang,
    defs: &mut Vec<RawDef>,
) {
    let (Some(raw_name), Some((kind, sub, start, end, line))) = (&ext.name, ext.def) else {
        return;
    };
    let (orig_name, inline_scope) = split_top_level_qualified(raw_name);
    let gate_ctx = crate::languages::DefCaptureGateContext {
        kind,
        sub,
        name: orig_name,
        name_node: ext.name_node,
        def_node: ext.def_node,
        source_bytes,
    };
    if lang.semantics().is_builtin_keyword(orig_name)
        || !lang.semantics().is_def_capture_allowed(&gate_ctx)
    {
        return;
    }
    let scope = inline_scope.or_else(|| {
        ext.name_node
            .and_then(|n| find_enclosing_scope(n, source_bytes, lang))
    });
    let (body_node, end) = lang.semantics().resolve_def_body_and_end(ext.def_node, end);
    let body_byte = body_node.map_or(0, find_def_body_start);
    let start_row = line.saturating_sub(1);
    let end_row = body_node.map_or(start_row, |n| n.end_position().row as u32);
    let loc = end_row.saturating_sub(start_row) + 1;
    let metrics = if kind == SymKind::Function || kind == SymKind::Method {
        body_node
            .map(|n| crate::metrics::compute_complexity_from_node(n, source_bytes, lang))
            .unwrap_or_default()
    } else {
        crate::metrics::ComplexityMetrics::default()
    };
    let (params, name) =
        lang.semantics()
            .resolve_def_params_and_name(ext.def_node, source_bytes, kind, orig_name);
    defs.push(RawDef {
        name,
        scope,
        kind,
        line,
        start_byte: start,
        body_byte,
        end_byte: end,
        loc,
        params,
        metrics,
    });
}

fn record_ref_item(
    ext: &MatchExtract<'_>,
    source_bytes: &[u8],
    lang: Lang,
    refs: &mut Vec<RawRef>,
) {
    let (Some(raw_name), Some((role, start, line))) = (&ext.name, ext.r#ref) else {
        return;
    };
    let (name, inline_qual) = if role == RefRole::Import {
        (raw_name.as_str(), None)
    } else {
        split_top_level_qualified(raw_name)
    };
    let is_pkg_prefix_extends = role == RefRole::Extends
        && ext
            .name_node
            .and_then(|n| n.next_sibling())
            .is_some_and(|s| s.kind() == ".");
    if is_pkg_prefix_extends
        || lang.semantics().is_builtin_keyword(name)
        || (role == RefRole::Call && lang.semantics().should_ignore_call(ext.name_node, name))
    {
        return;
    }

    let qualifier = inline_qual.or_else(|| {
        ext.name_node.and_then(|n| {
            if role == RefRole::Extends {
                find_enclosing_scope(n, source_bytes, lang)
            } else {
                find_call_qualifier(n, source_bytes, lang)
            }
        })
    });
    let shadow_applies = role == RefRole::Read
        || (role == RefRole::Call && lang.semantics().parameters_shadow_calls());
    if qualifier.is_none()
        && shadow_applies
        && ext
            .name_node
            .is_some_and(|n| !is_member_access(n) && is_enclosing_parameter(n, name, source_bytes))
    {
        return;
    }
    let (arg_count, arg_count_known) = ext
        .name_node
        .map(count_call_arguments)
        .unwrap_or((0, false));

    let callee_name = if role == RefRole::Call {
        lang.semantics()
            .format_callee_name(name, arg_count, arg_count_known)
    } else {
        name.to_string()
    };
    refs.push(RawRef {
        callee_name,
        qualifier,
        line,
        start_byte: start,
        role,
        arg_count,
        arg_count_known,
        lang: None,
    });
}

fn is_member_access(name_node: tree_sitter::Node<'_>) -> bool {
    name_node
        .prev_sibling()
        .is_some_and(|s| matches!(s.kind(), "." | "?." | "optional_chain" | "->" | "::"))
}

const NON_BINDING_PARAM_FIELDS: &[&str] = &[
    "value",
    "right",
    "type",
    "default_type",
    "bounds",
    "constraint",
    "decorator",
    "type_arguments",
];

/// True when a function or closure enclosing `node` declares a parameter named `name`.
fn is_enclosing_parameter(node: tree_sitter::Node<'_>, name: &str, source_bytes: &[u8]) -> bool {
    let mut cur = node.parent();
    while let Some(n) = cur {
        if let Some(params) = n.child_by_field_name("parameters")
            && node.start_byte() >= params.end_byte()
            && binds_name(params, name, source_bytes)
        {
            return true;
        }
        cur = n.parent();
    }
    false
}

fn binds_name(node: tree_sitter::Node<'_>, name: &str, source_bytes: &[u8]) -> bool {
    if node.child_count() == 0 {
        return node.utf8_text(source_bytes).is_ok_and(|t| t == name);
    }
    (0..node.child_count()).any(|i| {
        node.field_name_for_child(i as u32)
            .is_none_or(|f| !NON_BINDING_PARAM_FIELDS.contains(&f))
            && node
                .child(i)
                .is_some_and(|c| binds_name(c, name, source_bytes))
    })
}

fn is_macro_call_keyword(ident: &str, lang: Lang) -> bool {
    lang.semantics().is_builtin_keyword(ident)
        || matches!(
            ident,
            "if" | "for" | "while" | "switch" | "return" | "defined" | "typeof"
        )
}

fn scan_macro_body_calls(
    def_node: Option<tree_sitter::Node<'_>>,
    macro_name: &str,
    line: u32,
    start_byte: usize,
    lang: Lang,
    source_bytes: &[u8],
    refs: &mut Vec<RawRef>,
) {
    let Some(val_node) = def_node.and_then(|n| n.child_by_field_name("value")) else {
        return;
    };
    let Ok(text) = val_node.utf8_text(source_bytes) else {
        return;
    };
    for seg in text.split('(').collect::<Vec<_>>().windows(2) {
        let prefix = seg[0].trim_end();
        let ident: &str = prefix
            .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .next()
            .unwrap_or("");
        let valid = ident.len() >= 2
            && ident != macro_name
            && ident
                .bytes()
                .next()
                .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
            && !is_macro_call_keyword(ident, lang);
        if valid {
            refs.push(RawRef {
                callee_name: ident.to_string(),
                qualifier: None,
                line,
                start_byte: start_byte.saturating_add(1),
                role: RefRole::Call,
                arg_count: 0,
                arg_count_known: false,
                lang: None,
            });
        }
    }
}

fn record_extracted_item(
    ext: MatchExtract<'_>,
    source_bytes: &[u8],
    lang: Lang,
    defs: &mut Vec<RawDef>,
    refs: &mut Vec<RawRef>,
) {
    if let Some((kind, _, start, _, line)) = ext.def {
        record_def_item(&ext, source_bytes, lang, defs);
        if kind == SymKind::Macro
            && let Some(name) = &ext.name
        {
            scan_macro_body_calls(ext.def_node, name, line, start, lang, source_bytes, refs);
        }
    } else if ext.r#ref.is_some() {
        record_ref_item(&ext, source_bytes, lang, refs);
    }
}
const SQL_DISALLOWED_TARGETS: &[&str] = &[
    "select", "where", "set", "values", "null", "true", "false", "lateral", "unnest", "dual",
    "only", "stdin", "stdout", "json", "jsonb", "table", "index", "each", "row",
];

fn is_sql_string_node(kind: &str) -> bool {
    kind.contains("string") || kind == "raw_string_literal" || kind == "template_string"
}

fn has_sql_dml_anchor_bytes(bytes: &[u8]) -> bool {
    for kw in [b"select" as &[u8], b"insert", b"update", b"delete", b"join"] {
        if bytes.windows(kw.len()).any(|w| w.eq_ignore_ascii_case(kw)) {
            return true;
        }
    }
    false
}

fn has_sql_dml_anchor(text: &str) -> bool {
    has_sql_dml_anchor_bytes(text.as_bytes())
}

fn trim_sql_punct(s: &str) -> &str {
    s.trim_matches(|c: char| {
        matches!(
            c,
            '"' | '\'' | '`' | '(' | ')' | ',' | ';' | ':' | '[' | ']' | '\\'
        )
    })
}

fn clean_sql_token(tok: &str) -> &str {
    let t = trim_sql_punct(tok);
    let last = t.rsplit('.').next().unwrap_or(t);
    trim_sql_punct(last)
}

fn is_valid_sql_table_ident(ident: &str) -> bool {
    if ident.len() < 2
        || ident.starts_with('$')
        || !ident
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        || !ident
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return false;
    }
    !SQL_DISALLOWED_TARGETS
        .iter()
        .any(|&kw| ident.eq_ignore_ascii_case(kw))
}

fn classify_sql_keyword_pair(prev: &str, curr: &str) -> Option<RefRole> {
    if curr.eq_ignore_ascii_case("from") {
        if prev.eq_ignore_ascii_case("delete") {
            Some(RefRole::Write)
        } else if prev.eq_ignore_ascii_case("distinct") || prev.eq_ignore_ascii_case("extract") {
            None
        } else {
            Some(RefRole::Read)
        }
    } else if curr.eq_ignore_ascii_case("join") {
        Some(RefRole::Read)
    } else if (curr.eq_ignore_ascii_case("into") && prev.eq_ignore_ascii_case("insert"))
        || (curr.eq_ignore_ascii_case("update") && !prev.eq_ignore_ascii_case("for"))
    {
        Some(RefRole::Write)
    } else {
        None
    }
}

/// Names a literal declares as CTEs (`name AS (`), which are local to that query, not tables.
fn cte_names<'a>(tokens: &[&'a str]) -> Vec<&'a str> {
    tokens
        .windows(3)
        .filter(|w| {
            clean_sql_token(w[1]).eq_ignore_ascii_case("as")
                && (w[1].ends_with('(') || w[2].starts_with('('))
        })
        .map(|w| clean_sql_token(w[0]))
        .collect()
}

fn scan_sql_tokens_in_literal(text: &str, start_byte: usize, line: u32, refs: &mut Vec<RawRef>) {
    if !has_sql_dml_anchor(text) {
        return;
    }
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let ctes = cte_names(&tokens);
    for i in 0..tokens.len() {
        let prev = if i > 0 {
            clean_sql_token(tokens[i - 1])
        } else {
            ""
        };
        let curr = clean_sql_token(tokens[i]);
        let Some(role) = classify_sql_keyword_pair(prev, curr) else {
            continue;
        };
        let mut target_idx = i + 1;
        if target_idx < tokens.len()
            && clean_sql_token(tokens[target_idx]).eq_ignore_ascii_case("only")
        {
            target_idx += 1;
        }
        if let Some(&raw_target) = tokens.get(target_idx) {
            let table = clean_sql_token(raw_target);
            if is_valid_sql_table_ident(table)
                && !ctes.iter().any(|c| c.eq_ignore_ascii_case(table))
            {
                refs.push(RawRef {
                    callee_name: table.to_string(),
                    qualifier: None,
                    line,
                    start_byte,
                    role,
                    arg_count: 0,
                    arg_count_known: false,
                    lang: Some(Lang::Sql),
                });
            }
        }
    }
}

fn extract_embedded_sql_refs(
    node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
    refs: &mut Vec<RawRef>,
) {
    if is_sql_string_node(node.kind()) {
        if let Ok(text) = node.utf8_text(source_bytes) {
            let line = node.start_position().row as u32 + 1;
            scan_sql_tokens_in_literal(text, node.start_byte(), line, refs);
        }
        return;
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            extract_embedded_sql_refs(child, source_bytes, refs);
        }
    }
}

fn count_error_bytes(node: tree_sitter::Node<'_>) -> usize {
    if node.is_error() || node.kind() == "ERROR" {
        return node.end_byte().saturating_sub(node.start_byte()).max(1);
    }
    if !node.has_error() {
        return 0;
    }
    let mut total = 0usize;
    for i in 0..node.child_count() {
        if let Some(ch) = node.child(i) {
            total += count_error_bytes(ch);
        }
    }
    total
}

fn parse_with_macro_blank_fallback(
    parser: &mut tree_sitter::Parser,
    source_bytes: &[u8],
    lang: Lang,
) -> Option<tree_sitter::Tree> {
    let tree = parser.parse(source_bytes, None)?;
    if !tree.root_node().has_error() {
        return Some(tree);
    }
    let Some(blanked) = lang.semantics().blank_error_macros(source_bytes) else {
        return Some(tree);
    };
    let Some(reparsed) = parser.parse(&blanked, None) else {
        return Some(tree);
    };
    if count_error_bytes(reparsed.root_node()) < count_error_bytes(tree.root_node()) {
        Some(reparsed)
    } else {
        Some(tree)
    }
}

fn dedup_conditional_macros(defs: &mut Vec<RawDef>) {
    let mut seen_macros = std::collections::HashSet::new();
    defs.retain(|d| d.kind != SymKind::Macro || seen_macros.insert(d.name.clone()));
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

        let Some(tree) = parse_with_macro_blank_fallback(parser, source_bytes, lang) else {
            return FileParseResult::default();
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
        if lang.semantics().supports_embedded_sql() && has_sql_dml_anchor_bytes(source_bytes) {
            extract_embedded_sql_refs(tree.root_node(), source_bytes, &mut refs);
        }

        defs.sort_by(|a, b| {
            a.start_byte
                .cmp(&b.start_byte)
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| {
                    let a_prio = u8::from(a.kind != SymKind::Method);
                    let b_prio = u8::from(b.kind != SymKind::Method);
                    a_prio.cmp(&b_prio)
                })
        });
        defs.dedup_by(|a, b| a.start_byte == b.start_byte && a.name == b.name);
        dedup_conditional_macros(&mut defs);
        FileParseResult {
            line_count,
            byte_size: source_bytes.len() as u64,
            defs,
            refs,
        }
    })
}
