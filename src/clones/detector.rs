use super::token_stream::{
    compute_jaccard, compute_kgrams, compute_lcs_len, hash_tokens, tokenize_body,
};
use crate::domain::{IngestResult, Interner, NodeId, SymKind};
use std::collections::HashMap;

/// Classification of clone match type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloneType {
    Type1 = 1,
    Type2 = 2,
    Type3 = 3,
}

impl CloneType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        if self == Self::Type1 {
            "1"
        } else if self == Self::Type2 {
            "2"
        } else {
            "3"
        }
    }
}

/// Recognized programming idiom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloneIdiom {
    None,
    ThresholdLadder,
    SwitchNameTable,
    BuilderChain,
}

impl CloneIdiom {
    #[must_use]
    pub const fn as_str(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::ThresholdLadder => Some("threshold-ladder"),
            Self::SwitchNameTable => Some("switch-name-table"),
            Self::BuilderChain => Some("builder-chain"),
        }
    }
}

/// A detected clone group with its member symbols and metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct CloneGroup {
    pub members: Vec<NodeId>,
    pub clone_type: CloneType,
    pub token_count: usize,
    pub similarity: f64,
    pub idiom: CloneIdiom,
    pub tokens_saved: usize,
}

struct BodyTokens {
    id: NodeId,
    raw_hash: u64,
    norm_hash: u64,
    norm_tokens: Vec<String>,
    kgrams: Vec<u64>,
}

fn is_threshold_ladder(tokens: &[String]) -> bool {
    let if_count = tokens.iter().filter(|&t| t == "if").count();
    let ret_count = tokens.iter().filter(|&t| t == "return").count();
    if_count >= 2 && ret_count >= 2 && tokens.len() <= 80
}

fn is_switch_table(tokens: &[String]) -> bool {
    let case_count = tokens.iter().filter(|&t| t == "case" || t == "=>").count();
    let switch_or_match = tokens.iter().any(|t| t == "switch" || t == "match");
    switch_or_match && case_count >= 2 && tokens.len() <= 80
}

fn is_builder_chain(tokens: &[String]) -> bool {
    let dot_count = tokens.iter().filter(|&t| t == ".").count();
    let eq_count = tokens.iter().filter(|&t| t == "=").count();
    eq_count >= 3 && dot_count >= 3 && tokens.len() <= 80
}

fn classify_body_idiom(tokens: &[String]) -> CloneIdiom {
    if is_threshold_ladder(tokens) {
        CloneIdiom::ThresholdLadder
    } else if is_switch_table(tokens) {
        CloneIdiom::SwitchNameTable
    } else if is_builder_chain(tokens) {
        CloneIdiom::BuilderChain
    } else {
        CloneIdiom::None
    }
}

fn check_group_idiom(members: &[&BodyTokens]) -> CloneIdiom {
    let Some(first_member) = members.first() else {
        return CloneIdiom::None;
    };
    let first = classify_body_idiom(&first_member.norm_tokens);
    if first == CloneIdiom::None {
        return CloneIdiom::None;
    }
    for m in &members[1..] {
        if classify_body_idiom(&m.norm_tokens) != first {
            return CloneIdiom::None;
        }
    }
    first
}

fn get_or_load_file_bytes<'a>(
    file_id: usize,
    ingest: &IngestResult,
    interner: &Interner,
    sources: Option<&HashMap<String, Vec<u8>>>,
    file_cache: &'a mut [Option<Vec<u8>>],
) -> Option<&'a [u8]> {
    if file_cache[file_id].is_none() {
        let file_info = ingest.files.get(file_id)?;
        let file_path = interner.resolve(&file_info.path);
        let bytes = if let Some(map) = sources {
            map.get(file_path).cloned()
        } else {
            std::fs::read(file_path).ok()
        };
        file_cache[file_id] = bytes;
    }
    file_cache[file_id].as_deref()
}

fn extract_candidate_bodies(
    ingest: &IngestResult,
    interner: &Interner,
    min_tokens: usize,
    sources: Option<&HashMap<String, Vec<u8>>>,
) -> Vec<BodyTokens> {
    let mut candidates = Vec::new();
    let mut file_cache: Vec<Option<Vec<u8>>> = vec![None; ingest.files.len()];

    for sym in &ingest.symbols {
        if (sym.kind != SymKind::Function && sym.kind != SymKind::Method)
            || sym.end_byte <= sym.sig_start_byte
        {
            continue;
        }

        let fid = sym.file_id as usize;
        if fid >= file_cache.len() {
            continue;
        }

        let Some(file_bytes) =
            get_or_load_file_bytes(fid, ingest, interner, sources, &mut file_cache)
        else {
            continue;
        };

        let start = (sym.sig_start_byte as usize).min(file_bytes.len());
        let end = (sym.end_byte as usize).min(file_bytes.len());
        if start >= end {
            continue;
        }

        let body_bytes = &file_bytes[start..end];
        let (raw_tokens, norm_tokens) = tokenize_body(body_bytes, sym.lang);
        if norm_tokens.len() < min_tokens {
            continue;
        }

        let raw_hash = hash_tokens(&raw_tokens);
        let norm_hash = hash_tokens(&norm_tokens);
        let kgrams = compute_kgrams(&norm_tokens, 5);

        candidates.push(BodyTokens {
            id: sym.id,
            raw_hash,
            norm_hash,
            norm_tokens,
            kgrams,
        });
    }

    candidates
}

fn group_exact_clones(candidates: &[BodyTokens]) -> Vec<CloneGroup> {
    let mut by_norm: HashMap<u64, Vec<&BodyTokens>> = HashMap::new();
    for c in candidates {
        by_norm.entry(c.norm_hash).or_default().push(c);
    }

    let mut groups = Vec::new();
    for list in by_norm.into_values() {
        if list.len() < 2 {
            continue;
        }

        let first_raw = list[0].raw_hash;
        let is_type1 = list.iter().all(|c| c.raw_hash == first_raw);
        let clone_type = if is_type1 {
            CloneType::Type1
        } else {
            CloneType::Type2
        };

        let members: Vec<NodeId> = list.iter().map(|c| c.id).collect();
        let token_count = list[0].norm_tokens.len();
        let tokens_saved = (members.len() - 1) * token_count;
        let idiom = check_group_idiom(&list);
        groups.push(CloneGroup {
            members,
            clone_type,
            token_count,
            similarity: 1.0,
            idiom,
            tokens_saved,
        });
    }
    groups
}

fn intern_tokens_to_ids(a: &[String], b: &[String]) -> (Vec<u32>, Vec<u32>) {
    let mut dict: HashMap<String, u32> = HashMap::new();
    let mut next_id = 0u32;

    let mut map_slice = |slice: &[String]| -> Vec<u32> {
        let mut ids = Vec::with_capacity(slice.len());
        for s in slice {
            let id = if let Some(&id) = dict.get(s) {
                id
            } else {
                let id = next_id;
                next_id += 1;
                dict.insert(s.clone(), id);
                id
            };
            ids.push(id);
        }
        ids
    };
    let a_ids = map_slice(a);
    let b_ids = map_slice(b);
    (a_ids, b_ids)
}

fn evaluate_candidate_pair(ca: &BodyTokens, cb: &BodyTokens) -> Option<CloneGroup> {
    let len_a = ca.norm_tokens.len();
    let len_b = cb.norm_tokens.len();
    let min_len = len_a.min(len_b) as f64;
    let max_len = len_a.max(len_b) as f64;
    if min_len / max_len < 0.70 {
        return None;
    }

    if compute_jaccard(&ca.kgrams, &cb.kgrams) < 0.40 {
        return None;
    }

    let (a_ids, b_ids) = intern_tokens_to_ids(&ca.norm_tokens, &cb.norm_tokens);
    let lcs = compute_lcs_len(&a_ids, &b_ids);
    let similarity = (2.0 * lcs as f64) / (len_a + len_b) as f64;

    if (0.80..1.0).contains(&similarity) {
        let avg_tokens = (len_a + len_b) / 2;
        Some(CloneGroup {
            members: vec![ca.id, cb.id],
            clone_type: CloneType::Type3,
            token_count: avg_tokens,
            similarity,
            idiom: CloneIdiom::None,
            tokens_saved: avg_tokens,
        })
    } else {
        None
    }
}

fn find_type3_pairs(
    candidates: &[BodyTokens],
    exact_matched: &HashMap<NodeId, ()>,
) -> Vec<CloneGroup> {
    let mut gapped = Vec::new();

    for (i, ca) in candidates.iter().enumerate() {
        if exact_matched.contains_key(&ca.id) {
            continue;
        }

        for cb in &candidates[i + 1..] {
            if exact_matched.contains_key(&cb.id) {
                continue;
            }
            if let Some(group) = evaluate_candidate_pair(ca, cb) {
                gapped.push(group);
            }
        }
    }

    gapped
}

/// Detect all Type-1, Type-2, and Type-3 clones across the codebase.
#[must_use]
pub fn find_clones(
    ingest: &IngestResult,
    interner: &Interner,
    min_tokens: usize,
    sources: Option<&HashMap<String, Vec<u8>>>,
) -> Vec<CloneGroup> {
    let candidates = extract_candidate_bodies(ingest, interner, min_tokens, sources);
    let mut exact_groups = group_exact_clones(&candidates);

    let mut exact_matched = HashMap::new();
    for g in &exact_groups {
        for &m in &g.members {
            exact_matched.insert(m, ());
        }
    }

    let gapped_groups = find_type3_pairs(&candidates, &exact_matched);
    exact_groups.extend(gapped_groups);
    exact_groups.sort_by_key(|b| std::cmp::Reverse(b.tokens_saved));
    exact_groups
}

/// Format clone detection results as dense minified XML.
#[must_use]
pub fn render_clones_xml(
    groups: &[CloneGroup],
    ingest: &IngestResult,
    interner: &Interner,
) -> String {
    let total_saved: usize = groups.iter().map(|g| g.tokens_saved).sum();
    let mut out = format!(
        "<clones schema=\"ripwire.clones/v1\" groups=\"{}\" tokens_saved=\"{total_saved}\">\n",
        groups.len()
    );

    for g in groups {
        let idiom_attr = match g.idiom.as_str() {
            Some(name) => format!(" idiom=\"{name}\""),
            None => String::new(),
        };

        out.push_str(&format!(
            "  <group type=\"{}\" tokens=\"{}\" similarity=\"{:.2}\" saved=\"{}\"{idiom_attr}>\n",
            g.clone_type.as_str(),
            g.token_count,
            g.similarity,
            g.tokens_saved
        ));

        for &id in &g.members {
            if let Some(sym) = ingest.symbols.get(id as usize) {
                let name = interner.resolve(&sym.name);
                let p = if let Some(fi) = ingest.files.get(sym.file_id as usize) {
                    format!("{}:{}", interner.resolve(&fi.path), sym.line)
                } else {
                    format!("line:{}", sym.line)
                };
                out.push_str(&format!("    <f n=\"{name}\" p=\"{p}\" />\n"));
            }
        }
        out.push_str("  </group>\n");
    }

    out.push_str("</clones>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Lang;
    use crate::ingest::{LanguageRegistry, ingest_in_memory_files};

    #[test]
    fn test_clone_type1_and_type2_detection() {
        let interner = Interner::new();
        let registry = LanguageRegistry::new().unwrap();

        // Exact Type-1 functions and renamed Type-2 functions
        let code1 = b"fn func_one(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n    let prod = a * b;\n    return sum + prod + 100;\n}\n";
        let code2 = b"fn func_two(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n    let prod = a * b;\n    return sum + prod + 100;\n}\n";
        let code3 = b"fn func_three(x: i32, y: i32) -> i32 {\n    let alpha = x + y;\n    let beta = x * y;\n    return alpha + beta + 100;\n}\n";

        let mut sources = HashMap::new();
        sources.insert("src/a.rs".into(), code1.to_vec());
        sources.insert("src/b.rs".into(), code2.to_vec());
        sources.insert("src/c.rs".into(), code3.to_vec());

        let ingest = ingest_in_memory_files(
            &[
                ("src/a.rs", Lang::Rust, code1),
                ("src/b.rs", Lang::Rust, code2),
                ("src/c.rs", Lang::Rust, code3),
            ],
            &registry,
            &interner,
        );

        let clones = find_clones(&ingest, &interner, 10, Some(&sources));
        assert!(!clones.is_empty(), "Should detect clone group");
        assert_eq!(clones[0].members.len(), 3);
        assert_eq!(clones[0].clone_type, CloneType::Type2);
        let xml = render_clones_xml(&clones, &ingest, &interner);
        assert!(xml.contains("<clones schema=\"ripwire.clones/v1\""));
        assert!(xml.contains("<f n=\"func_one\""));
        assert!(xml.contains("<group type=\"2\""));
        assert_eq!(CloneIdiom::None.as_str(), None);
        assert_eq!(
            CloneIdiom::ThresholdLadder.as_str(),
            Some("threshold-ladder")
        );
        assert_eq!(
            CloneIdiom::SwitchNameTable.as_str(),
            Some("switch-name-table")
        );
        assert_eq!(CloneIdiom::BuilderChain.as_str(), Some("builder-chain"));
    }
}
