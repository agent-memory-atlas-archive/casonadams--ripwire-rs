use crate::domain::Lang;
use crate::ingest::LanguageRegistry;
use std::path::Path;
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

pub(crate) fn read_head_blob_bytes(repo_path: &Path, rel_path: &str) -> Option<Vec<u8>> {
    let repo = gix::discover(repo_path).ok()?;
    let head = repo.head_commit().ok()?;
    let tree = head.tree().ok()?;
    if let Ok(Some(entry)) = tree.lookup_entry_by_path(rel_path)
        && let Ok(obj) = entry.object()
    {
        return Some(obj.data.to_vec());
    }
    let workdir = repo.workdir()?;
    let full_target = repo_path.canonicalize().ok()?.join(rel_path);
    let canon_workdir = workdir.canonicalize().ok()?;
    let repo_rel = full_target.strip_prefix(&canon_workdir).ok()?;
    let entry = tree.lookup_entry_by_path(repo_rel).ok()??;
    let obj = entry.object().ok()?;
    Some(obj.data.to_vec())
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
            params = extract_param_count(node);
            def_node = Some(node);
        }
    }
    let dn = def_node?;
    let n = name?;
    let is_pub = resolve_head_visibility(Some(&n), Some(dn), bytes, lang);
    Some((n, params, is_pub))
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

    let registry = LanguageRegistry::new().map_err(|e| e.to_string())?;
    let Some((ts_lang, query)) = registry.get(lang) else {
        return Ok(None);
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(ts_lang).is_err() {
        return Ok(None);
    }

    let Some(tree) = parser.parse(&bytes, None) else {
        return Ok(None);
    };

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), bytes.as_slice());

    while let Some(m) = matches.next() {
        if let Some((n, params, is_public)) = parse_head_signature_match(m, query, &bytes, lang)
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

fn is_optional_param_decl(kind: &str) -> bool {
    kind.contains("parameter") || kind == "constructor_param"
}

fn is_variadic_param_kind(kind: &str) -> bool {
    kind.contains("rest")
        || kind.contains("variadic")
        || kind.contains("splat")
        || kind == "spread_element"
}

fn count_single_param(param: tree_sitter::Node<'_>, is_kt: bool) -> u16 {
    let kind = param.kind();
    if is_variadic_param_kind(kind) {
        return 64;
    }
    if kind == "optional_formal_parameters" {
        return (0..param.named_child_count())
            .filter_map(|i| param.named_child(i))
            .filter(|p| is_optional_param_decl(p.kind()))
            .count() as u16;
    }
    u16::from(if is_kt {
        kind == "parameter"
    } else {
        !kind.contains("comment")
    })
}

fn count_param_children(child: tree_sitter::Node<'_>) -> u16 {
    let is_kt = child.kind() == "function_value_parameters";
    (0..child.named_child_count())
        .filter_map(|j| child.named_child(j))
        .map(|p| count_single_param(p, is_kt))
        .sum()
}

fn find_callable_descendant(
    node: tree_sitter::Node<'_>,
    depth: u8,
) -> Option<tree_sitter::Node<'_>> {
    if depth == 0 {
        return None;
    }
    for i in 0..node.named_child_count() {
        let Some(ch) = node.named_child(i) else {
            continue;
        };
        if matches!(
            ch.kind(),
            "arrow_function" | "function_expression" | "function_declarator"
        ) {
            return Some(ch);
        }
        if matches!(
            ch.kind(),
            "variable_declarator"
                | "as_expression"
                | "satisfies_expression"
                | "parenthesized_expression"
        ) && let Some(found) = find_callable_descendant(ch, depth - 1)
        {
            return Some(found);
        }
    }
    None
}

#[must_use]
pub fn unwrap_signature_node(def_node: tree_sitter::Node<'_>) -> tree_sitter::Node<'_> {
    if matches!(def_node.kind(), "method_signature" | "declaration") {
        for i in 0..def_node.named_child_count() {
            if let Some(ch) = def_node.named_child(i)
                && ch.kind().contains("signature")
            {
                return ch;
            }
        }
    }
    if let Some(callable) = find_callable_descendant(def_node, 4) {
        return callable;
    }
    def_node
}

fn is_value_parameter_list_kind(kind: &str) -> bool {
    (kind.contains("parameter")
        && !kind.contains("type_parameter")
        && kind != "template_parameter_list")
        || kind == "params"
}

#[must_use]
pub fn extract_param_count(def_node: tree_sitter::Node<'_>) -> u16 {
    let node = unwrap_signature_node(def_node);
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i)
            && is_value_parameter_list_kind(child.kind())
        {
            return count_param_children(child);
        }
    }
    if node.kind() == "arrow_function" && node.child_by_field_name("parameter").is_some() {
        return 1;
    }
    0
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
