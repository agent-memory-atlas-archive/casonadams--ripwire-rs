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
    if matches!(
        lang,
        Lang::Dart
            | Lang::Python
            | Lang::Ruby
            | Lang::Elixir
            | Lang::Lua
            | Lang::Bash
            | Lang::GDScript
            | Lang::Go
    ) {
        return name.is_none_or(|n| is_symbol_name_public(n, lang));
    }
    def_node.is_none_or(|dn| check_node_visibility(dn, bytes, lang))
}

fn parse_head_signature_match(
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
pub fn load_head_symbol_contract(
    repo_path: &Path,
    rel_path: &str,
    sym_name: &str,
    lang: Lang,
) -> Result<Option<SignatureContract>, String> {
    let Some(bytes) = read_head_blob_bytes(repo_path, rel_path) else {
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

fn count_single_param(param: tree_sitter::Node<'_>, is_kt: bool) -> u16 {
    if param.kind() == "optional_formal_parameters" {
        return (0..param.named_child_count())
            .filter_map(|i| param.named_child(i))
            .filter(|p| is_optional_param_decl(p.kind()))
            .count() as u16;
    }
    u16::from(if is_kt {
        param.kind() == "parameter"
    } else {
        !param.kind().contains("comment")
    })
}

fn count_param_children(child: tree_sitter::Node<'_>) -> u16 {
    let is_kt = child.kind() == "function_value_parameters";
    (0..child.named_child_count())
        .filter_map(|j| child.named_child(j))
        .map(|p| count_single_param(p, is_kt))
        .sum()
}

#[must_use]
fn unwrap_signature_node(def_node: tree_sitter::Node<'_>) -> tree_sitter::Node<'_> {
    if matches!(def_node.kind(), "method_signature" | "declaration") {
        for i in 0..def_node.named_child_count() {
            if let Some(ch) = def_node.named_child(i)
                && ch.kind().contains("signature")
            {
                return ch;
            }
        }
    }
    def_node
}

#[must_use]
pub fn extract_param_count(def_node: tree_sitter::Node<'_>) -> u16 {
    let node = unwrap_signature_node(def_node);
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            let kind = child.kind();
            if kind.contains("parameter") || kind == "params" {
                return count_param_children(child);
            }
        }
    }
    0
}

fn is_go_public(def_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
    let Some(name_node) = def_node.child_by_field_name("name") else {
        return false;
    };
    let Ok(text) = name_node.utf8_text(source_bytes) else {
        return false;
    };
    text.chars().next().is_some_and(|c| c.is_uppercase())
}

fn has_visibility_modifier(child: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
    let kind = child.kind();
    if (kind == "visibility_modifier" || kind == "accessibility_modifier")
        && let Ok(text) = child.utf8_text(source_bytes)
    {
        return text.contains("pub") || text.contains("public");
    }
    false
}

fn has_any_visibility_modifier(def_node: tree_sitter::Node<'_>, source_bytes: &[u8]) -> bool {
    for i in 0..def_node.child_count() {
        if let Some(child) = def_node.child(i)
            && has_visibility_modifier(child, source_bytes)
        {
            return true;
        }
    }
    false
}

#[must_use]
pub fn is_symbol_name_public(sym_name: &str, lang: Lang) -> bool {
    if lang == Lang::Go {
        sym_name.chars().next().is_some_and(|c| c.is_uppercase())
    } else if matches!(
        lang,
        Lang::Dart
            | Lang::Python
            | Lang::Ruby
            | Lang::Elixir
            | Lang::Lua
            | Lang::Bash
            | Lang::GDScript
    ) {
        !sym_name.starts_with('_') && !sym_name.starts_with('#')
    } else {
        !sym_name.starts_with('_')
    }
}

#[must_use]
pub fn check_node_visibility(
    def_node: tree_sitter::Node<'_>,
    source_bytes: &[u8],
    lang: Lang,
) -> bool {
    if lang == Lang::Go {
        is_go_public(def_node, source_bytes)
    } else if matches!(
        lang,
        Lang::Dart
            | Lang::Python
            | Lang::Ruby
            | Lang::Elixir
            | Lang::Lua
            | Lang::Bash
            | Lang::GDScript
    ) {
        let node = unwrap_signature_node(def_node);
        let is_private = node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source_bytes).ok())
            .is_some_and(|s| s.starts_with('_'));
        !is_private
    } else if matches!(
        lang,
        Lang::Rust | Lang::Java | Lang::CSharp | Lang::Php | Lang::Kotlin | Lang::Swift
    ) {
        has_any_visibility_modifier(def_node, source_bytes)
    } else {
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_contracts_scenarios() {
        let baseline = SignatureContract {
            name: "fetch".to_string(),
            params: 2,
            is_public: false,
        };

        // 1. Unchanged
        let same = SignatureContract {
            name: "fetch".to_string(),
            params: 2,
            is_public: false,
        };
        assert_eq!(
            compare_contracts(&same, Some(&baseline)),
            ContractStatus::Unchanged
        );

        // 2. Arity change
        let arity_broken = SignatureContract {
            name: "fetch".to_string(),
            params: 3,
            is_public: false,
        };
        assert_eq!(
            compare_contracts(&arity_broken, Some(&baseline)),
            ContractStatus::ContractChange {
                param_changed: true,
                public_changed: false,
                was_params: 2,
                now_params: 3,
                was_public: false,
                now_public: false,
            }
        );

        // 3. Visibility change
        let pub_changed = SignatureContract {
            name: "fetch".to_string(),
            params: 2,
            is_public: true,
        };
        assert_eq!(
            compare_contracts(&pub_changed, Some(&baseline)),
            ContractStatus::ContractChange {
                param_changed: false,
                public_changed: true,
                was_params: 2,
                now_params: 2,
                was_public: false,
                now_public: true,
            }
        );

        // 4. New symbol
        assert_eq!(compare_contracts(&same, None), ContractStatus::NewSymbol);
    }

    #[test]
    fn test_extract_param_and_visibility() {
        let code = b"pub fn example(a: i32, b: i32) {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let fn_node = tree.root_node().child(0).unwrap();
        assert_eq!(extract_param_count(fn_node), 2);
        assert!(check_node_visibility(fn_node, code, Lang::Rust));
    }

    #[test]
    fn test_dart_private_method_visibility_and_named_params() {
        let dart_code = br#"
class SwapRecord {
  SwapRecord(this.a, this.b, {required this.c, this.d, this.e});
  Future<void> run() async {
    await _prepare("first", y: 2);
  }
  Future<void> _prepare(String x, {int y = 1, bool z = false}) async {}
}
void makeSwap() {
  SwapRecord(1, 2, c: 3, d: 4);
}
"#;
        let reg = LanguageRegistry::new().unwrap();
        let (ts_lang, query) = reg.get(Lang::Dart).unwrap();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(ts_lang).unwrap();
        let tree = parser.parse(dart_code, None).unwrap();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), dart_code.as_slice());

        let mut found_prepare = false;
        let mut found_ctor = false;
        while let Some(m) = matches.next() {
            let Some((name, params, is_pub)) =
                parse_head_signature_match(m, query, dart_code, Lang::Dart)
            else {
                continue;
            };
            if name == "_prepare" {
                found_prepare = true;
                assert!(!is_pub, "private Dart method _prepare must not be public");
                assert_eq!(params, 3, "1 positional + 2 named parameters");
            } else if name == "SwapRecord" && params > 0 {
                found_ctor = true;
                assert!(is_pub);
                assert_eq!(params, 5, "2 positional + 3 named parameters");
            }
        }
        assert!(found_prepare);
        assert!(found_ctor);
    }
}
