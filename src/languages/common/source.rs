use crate::domain::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRegion {
    Code,
    Comment,
    Literal,
}

pub fn node_region(kind: &str) -> Option<SourceRegion> {
    if kind.contains("comment") {
        return Some(SourceRegion::Comment);
    }
    match kind {
        "string"
        | "string_literal"
        | "raw_string_literal"
        | "interpreted_string_literal"
        | "template_string"
        | "char_literal"
        | "character_literal"
        | "regex"
        | "regex_literal"
        | "heredoc_body"
        | "string_content"
        | "string_fragment"
        | "string_value"
        | "quoted_attribute_value"
        | "encapsed_string"
        | "nowdoc_string"
        | "heredoc" => Some(SourceRegion::Literal),
        "interpolation" | "template_substitution" | "interpolation_expression" => {
            Some(SourceRegion::Code)
        }
        _ => None,
    }
}

fn advance(cursor: &mut tree_sitter::TreeCursor<'_>) -> bool {
    if cursor.goto_first_child() {
        return true;
    }
    loop {
        if cursor.goto_next_sibling() {
            return true;
        }
        if !cursor.goto_parent() {
            return false;
        }
    }
}

#[must_use]
pub fn source_regions(src: &str, lang: Lang, path: &str) -> Vec<SourceRegion> {
    let Some(grammar) = lang.semantics().source_grammar(path) else {
        return vec![SourceRegion::Code; src.len()];
    };
    crate::ingest::registry::PARSER_STATE.with(|state| {
        let mut state = state.borrow_mut();
        let parser = &mut state.0;
        parser.set_language(grammar).expect("supported grammar");
        parser.parse(src, None).map_or_else(
            || vec![SourceRegion::Code; src.len()],
            |tree| source_regions_in_tree(&tree, src.len(), lang),
        )
    })
}

#[must_use]
pub fn source_regions_in_tree(
    tree: &tree_sitter::Tree,
    len: usize,
    lang: Lang,
) -> Vec<SourceRegion> {
    let semantics = lang.semantics();
    let default_region = if semantics.is_data_format() {
        SourceRegion::Literal
    } else {
        SourceRegion::Code
    };
    let mut regions = vec![default_region; len];
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        if let Some(region) = semantics.source_region(node.kind()) {
            regions[node.byte_range()].fill(region);
        }
        if !advance(&mut cursor) {
            break;
        }
    }
    regions
}
