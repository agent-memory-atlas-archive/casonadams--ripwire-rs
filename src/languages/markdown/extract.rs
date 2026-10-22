use crate::ingest::{FileParseResult, RawDef, RawRef};
use crate::domain::{RefRole, SymKind};
use std::cell::RefCell;
use tree_sitter::{Language, Node, Parser};

thread_local! {
    static MD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// Clean heading text by stripping carriage returns and trailing hashes/spaces.
fn clean_heading_text(raw: &str) -> String {
    let mut name = String::with_capacity(raw.len());
    for c in raw.chars() {
        if c != '\r' {
            name.push(if c == '\n' { ' ' } else { c });
        }
    }
    let trimmed = name.trim();
    trimmed.trim_matches('#').trim().to_string()
}

fn determine_atx_heading(node: Node<'_>, source_bytes: &[u8]) -> Option<(u32, String)> {
    let mut level = 1u32;
    let mut text = String::new();
    for i in 0..node.child_count() {
        let Some(child) = node.child(i) else { continue };
        let ckind = child.kind();
        if ckind.starts_with("atx_h") && ckind.ends_with("_marker") {
            if let Ok(marker) = child.utf8_text(source_bytes) {
                level = marker.chars().filter(|&c| c == '#').count() as u32;
            }
        } else if ckind == "heading_content"
            && let Ok(content) = child.utf8_text(source_bytes)
        {
            text = clean_heading_text(content);
        }
    }
    if text.is_empty()
        && let Ok(raw) = node.utf8_text(source_bytes)
    {
        text = clean_heading_text(raw);
    }
    Some((level.clamp(1, 6), text))
}

fn extract_paragraph_inline_text(child: Node<'_>, source_bytes: &[u8]) -> Option<String> {
    for j in 0..child.named_child_count() {
        let inline = child.named_child(j)?;
        if inline.kind() == "inline" {
            let content = inline.utf8_text(source_bytes).ok()?;
            return Some(clean_heading_text(content));
        }
    }
    child.utf8_text(source_bytes).ok().map(clean_heading_text)
}

fn determine_setext_heading(node: Node<'_>, source_bytes: &[u8]) -> Option<(u32, String)> {
    let mut level = 1u32;
    let mut text = String::new();
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i) else {
            continue;
        };
        match child.kind() {
            "setext_h1_underline" => level = 1,
            "setext_h2_underline" => level = 2,
            "paragraph" => {
                if let Some(t) = extract_paragraph_inline_text(child, source_bytes) {
                    text = t;
                }
            }
            _ => {}
        }
    }
    if text.is_empty() {
        None
    } else {
        Some((level, text))
    }
}

fn determine_heading_level(node: Node<'_>, source_bytes: &[u8]) -> Option<(u32, String)> {
    match node.kind() {
        "atx_heading" => determine_atx_heading(node, source_bytes),
        "setext_heading" => determine_setext_heading(node, source_bytes),
        _ => None,
    }
}

struct HeadingEntry {
    level: u32,
    name: String,
    line: u32,
    start_byte: usize,
}

fn walk_markdown_tree<'a>(node: Node<'a>, source_bytes: &[u8], headings: &mut Vec<HeadingEntry>) {
    let kind = node.kind();
    if kind == "fenced_code_block" || kind == "indented_code_block" || kind == "html_block" {
        return;
    }

    if let Some((level, name)) = determine_heading_level(node, source_bytes)
        && !name.is_empty()
    {
        headings.push(HeadingEntry {
            level,
            name,
            line: node.start_position().row as u32 + 1,
            start_byte: node.start_byte(),
        });
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            walk_markdown_tree(child, source_bytes, headings);
        }
    }
}

fn scan_line_links(line: &str, line_num: u32, line_start: usize, refs: &mut Vec<RawRef>) {
    let bytes = line.as_bytes();
    let mut b = 0;
    while b + 1 < bytes.len() {
        if bytes[b] != b'[' {
            b += 1;
            continue;
        }
        if bytes[b + 1] == b'[' {
            b += 2;
            continue;
        }
        let Some(close_rel) = bytes[b + 1..].iter().position(|&c| c == b']') else {
            break;
        };
        let close = b + 1 + close_rel;
        if close + 1 < bytes.len()
            && bytes[close + 1] == b'('
            && let Some(rp_rel) = bytes[close + 2..].iter().position(|&c| c == b')')
        {
            let rp = close + 2 + rp_rel;
            let target = line[close + 2..rp].trim();
            let clean_target = target.trim_start_matches('#');
            if !clean_target.is_empty() {
                refs.push(RawRef {
                    callee_name: clean_target.to_string(),
                    qualifier: None,
                    line: line_num,
                    name_line: line_num,
                    start_byte: line_start + b,
                    role: RefRole::Call,
                    arg_count: 0,
                    arg_count_known: false,
                    lang: None,
                    receiver: None,
                    receiver_type: None,
                    callable_field: false,
                });
            }
            b = rp + 1;
            continue;
        }
        b = close + 1;
    }
}

fn assemble_defs(headings: &[HeadingEntry], rel_path: &str, source_len: usize) -> Vec<RawDef> {
    let mut defs = Vec::with_capacity(headings.len() + 1);
    let stem = std::path::Path::new(rel_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("doc");

    defs.push(RawDef {
        name: stem.to_string(),
        scope: None,
        kind: SymKind::Section,
        line: 1,
        start_byte: 0,
        body_byte: 0,
        end_byte: source_len,
        loc: 1,
        params: 0,
        metrics: crate::metrics::ComplexityMetrics::default(),
        immutable: false,
    });

    for (i, h) in headings.iter().enumerate() {
        let mut end_byte = source_len;
        for next in &headings[i + 1..] {
            if next.level <= h.level {
                end_byte = next.start_byte;
                break;
            }
        }

        let mut scope = None;
        for prev in headings[..i].iter().rev() {
            if prev.level < h.level {
                scope = Some(prev.name.clone());
                break;
            }
        }

        defs.push(RawDef {
            name: h.name.clone(),
            scope,
            kind: SymKind::Section,
            line: h.line,
            start_byte: h.start_byte,
            body_byte: h.start_byte,
            end_byte,
            loc: 1,
            params: 0,
            metrics: crate::metrics::ComplexityMetrics::default(),
            immutable: false,
        });
    }

    defs
}

/// Extract symbols and references from Markdown document AST.
#[must_use]
pub fn extract_markdown(
    source_bytes: &[u8],
    rel_path: &str,
    language: &Language,
) -> FileParseResult {
    let tree = MD_PARSER.with(|cell| {
        let parser = &mut *cell.borrow_mut();
        if parser.set_language(language).is_err() {
            return None;
        }
        parser.parse(source_bytes, None)
    });
    let Some(tree) = tree else {
        return FileParseResult::default();
    };

    let mut headings = Vec::new();
    walk_markdown_tree(tree.root_node(), source_bytes, &mut headings);

    let mut refs = Vec::new();
    let mut line_num = 1;
    let mut line_start = 0;
    if let Ok(src_str) = std::str::from_utf8(source_bytes) {
        for line in src_str.lines() {
            scan_line_links(line, line_num, line_start, &mut refs);
            line_start += line.len() + 1;
            line_num += 1;
        }
    }
    let defs = assemble_defs(&headings, rel_path, source_bytes.len());
    let line_count = source_bytes.iter().filter(|&&b| b == b'\n').count() as u32 + 1;

    FileParseResult {
        line_count,
        byte_size: source_bytes.len() as u64,
        defs,
        refs,
        foreign_exports: Vec::new(),
        callable_bindings: Vec::new(),
        implementations: Vec::new(),
    }
}
