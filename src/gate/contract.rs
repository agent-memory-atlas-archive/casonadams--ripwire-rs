use crate::domain::Lang;
use crate::ingest::LanguageRegistry;
use crate::languages::SyntaxKinds;
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;

/// Function signature contract defining parameter count and public visibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureContract {
    pub name: String,
    pub params: u16,
    pub is_public: bool,
}

/// Comparison result between current AST signature and baseline (git HEAD).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractStatus {
    Unchanged,
    ContractChange {
        param_changed: bool,
        public_changed: bool,
        was_params: u16,
        now_params: u16,
        was_public: bool,
        now_public: bool,
    },
    NewSymbol,
    NoBaseline,
}

impl std::fmt::Display for ContractStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Unchanged => "unchanged",
            Self::ContractChange { .. } => "contract-change",
            Self::NewSymbol => "new-symbol",
            Self::NoBaseline => "no-baseline",
        };
        write!(f, "{s}")
    }
}

/// Compare working tree signature contract against a baseline.
#[must_use]
pub fn compare_contracts(
    now: &SignatureContract,
    was: Option<&SignatureContract>,
) -> ContractStatus {
    let Some(baseline) = was else {
        return ContractStatus::NewSymbol;
    };

    let param_changed = now.params != baseline.params;
    let public_changed = now.is_public != baseline.is_public;

    if !param_changed && !public_changed {
        ContractStatus::Unchanged
    } else {
        ContractStatus::ContractChange {
            param_changed,
            public_changed,
            was_params: baseline.params,
            now_params: now.params,
            was_public: baseline.is_public,
            now_public: now.is_public,
        }
    }
}

fn tree_entry_blob(tree: &gix::Tree<'_>, path: impl AsRef<Path>) -> Option<Vec<u8>> {
    let entry = tree.lookup_entry_by_path(path).ok()??;
    let obj = entry.object().ok()?;
    Some(obj.data.to_vec())
}

fn resolve_repo_relative_path(repo_path: &Path, workdir: &Path, rel_path: &str) -> Option<PathBuf> {
    let full_target = repo_path.canonicalize().ok()?.join(rel_path);
    let canon_workdir = workdir.canonicalize().ok()?;
    let repo_rel = full_target.strip_prefix(&canon_workdir).ok()?;
    Some(repo_rel.to_path_buf())
}

pub(crate) fn read_head_blob_bytes(repo_path: &Path, rel_path: &str) -> Option<Vec<u8>> {
    let repo = gix::discover(repo_path).ok()?;
    let tree = repo.head_commit().ok()?.tree().ok()?;
    tree_entry_blob(&tree, rel_path).or_else(|| {
        let repo_rel = resolve_repo_relative_path(repo_path, repo.workdir()?, rel_path)?;
        tree_entry_blob(&tree, repo_rel)
    })
}

fn resolve_head_visibility(
    name: Option<&str>,
    def_node: Option<tree_sitter::Node<'_>>,
    bytes: &[u8],
    lang: Lang,
) -> bool {
    if lang.semantics().visibility_mode() == crate::languages::VisibilityMode::ByNameConvention {
        return name.is_none_or(|n| is_symbol_name_public(n, lang));
    }
    def_node.is_none_or(|dn| check_node_visibility(dn, bytes, lang))
}

pub fn parse_head_signature_match(
    m: &tree_sitter::QueryMatch<'_, '_>,
    query: &tree_sitter::Query,
    bytes: &[u8],
    lang: Lang,
) -> Option<(String, u16, bool)> {
    let mut name = None;
    let mut params = 0u16;
    let mut def_node = None;

    for capture in m.captures {
        let cap_name = query.capture_names()[capture.index as usize];
        let node = capture.node;
        if cap_name == "name" {
            if let Ok(text) = node.utf8_text(bytes) {
                name = Some(text.to_string());
            }
        } else if cap_name.starts_with("definition.") {
            params = extract_param_count(node, lang.semantics().syntax_kinds());
            def_node = Some(node);
        }
    }
    let dn = def_node?;
    let n = name?;
    let is_pub = resolve_head_visibility(Some(&n), Some(dn), bytes, lang);
    Some((n, params, is_pub))
}

/// Parse a symbol's contract from source bytes in memory.
pub fn parse_symbol_contract_from_bytes(
    bytes: &[u8],
    sym_name: &str,
    lang: Lang,
) -> Result<Option<SignatureContract>, String> {
    let registry = LanguageRegistry::new().map_err(|e| e.to_string())?;
    let Some((ts_lang, query)) = registry.get(lang) else {
        return Ok(None);
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(ts_lang).is_err() {
        return Ok(None);
    }

    let Some(tree) = parser.parse(bytes, None) else {
        return Ok(None);
    };

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), bytes);

    while let Some(m) = matches.next() {
        if let Some((n, params, is_public)) = parse_head_signature_match(m, query, bytes, lang)
            && n == sym_name
        {
            return Ok(Some(SignatureContract {
                name: n,
                params,
                is_public,
            }));
        }
    }

    Ok(None)
}

/// Load a symbol's contract from the git HEAD commit.
pub fn load_head_symbol_contract<P: AsRef<Path>>(
    repo_path: P,
    rel_path: &str,
    sym_name: &str,
    lang: Lang,
) -> Result<Option<SignatureContract>, String> {
    let Some(bytes) = read_head_blob_bytes(repo_path.as_ref(), rel_path) else {
        return Ok(None);
    };
    parse_symbol_contract_from_bytes(&bytes, sym_name, lang)
}

fn is_variadic_param_kind(kind: &str) -> bool {
    ["rest", "variadic", "splat", "spread"]
        .iter()
        .any(|marker| kind.contains(marker))
}

fn count_single_param(param: tree_sitter::Node<'_>, strict: bool, kinds: &SyntaxKinds) -> u16 {
    let kind = param.kind();
    if is_variadic_param_kind(kind) {
        return 64;
    }
    if kinds.optional_param_groups.contains(&kind) {
        let mut cursor = param.walk();
        return param
            .named_children(&mut cursor)
            .filter(|p| p.kind().contains("param"))
            .count() as u16;
    }
    u16::from(if strict {
        kind == "parameter"
    } else {
        !kind.contains("comment")
    })
}

fn count_param_children(list: tree_sitter::Node<'_>, kinds: &SyntaxKinds) -> u16 {
    let strict = kinds.strict_param_lists.contains(&list.kind());
    let mut cursor = list.walk();
    list.named_children(&mut cursor)
        .map(|p| count_single_param(p, strict, kinds))
        .sum()
}

fn find_callable_descendant<'a>(
    node: tree_sitter::Node<'a>,
    depth: u8,
    kinds: &SyntaxKinds,
) -> Option<tree_sitter::Node<'a>> {
    if depth == 0 {
        return None;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find_map(|ch| {
        if kinds.callables.contains(&ch.kind()) {
            Some(ch)
        } else if kinds.callable_wrappers.contains(&ch.kind()) {
            find_callable_descendant(ch, depth - 1, kinds)
        } else {
            None
        }
    })
}

#[must_use]
pub fn unwrap_signature_node<'a>(
    def_node: tree_sitter::Node<'a>,
    kinds: &SyntaxKinds,
) -> tree_sitter::Node<'a> {
    if kinds.signature_holders.contains(&def_node.kind()) {
        let mut cursor = def_node.walk();
        if let Some(signature) = def_node
            .named_children(&mut cursor)
            .find(|ch| ch.kind().contains("signature"))
        {
            return signature;
        }
    }
    find_callable_descendant(def_node, 4, kinds).unwrap_or(def_node)
}

fn is_value_parameter_list_kind(kind: &str) -> bool {
    kind.contains("parameter") && !kind.contains("type_parameter") && !kind.starts_with("template_")
}

#[must_use]
pub fn extract_param_count(def_node: tree_sitter::Node<'_>, kinds: &SyntaxKinds) -> u16 {
    let node = unwrap_signature_node(def_node, kinds);
    let mut cursor = node.walk();
    if let Some(list) = node
        .children(&mut cursor)
        .find(|child| is_value_parameter_list_kind(child.kind()))
    {
        return count_param_children(list, kinds);
    }
    let single_param =
        kinds.callables.contains(&node.kind()) && node.child_by_field_name("parameter").is_some();
    u16::from(single_param)
}

#[must_use]
pub fn is_symbol_name_public(sym_name: &str, lang: Lang) -> bool {
    lang.semantics().is_symbol_name_public(sym_name)
}

#[must_use]
pub fn check_node_visibility(
    def_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
    lang: Lang,
) -> bool {
    lang.semantics()
        .check_node_visibility(def_node, source_bytes)
}
