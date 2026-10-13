use super::token_stream::{
    compute_jaccard, compute_kgrams_ids, compute_lcs_len, hash_tokens, token_id, tokenize_body,
};
use crate::domain::{IngestResult, Interner, NodeId, SymKind};
use rayon::prelude::*;
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
    norm_token_ids: Vec<u32>,
    kgrams: Vec<u64>,
}

const ID_IF: u32 = 0x3938_6e06;
const ID_RETURN: u32 = 0x85ee_37bf;
const ID_CASE: u32 = 0x9b25_38b1;
const ID_ARROW: u32 = 0x91f4_de62;
const ID_SWITCH: u32 = 0x93e0_5f71;
const ID_MATCH: u32 = 0x7e20_2f96;
const ID_DOT: u32 = 0x2b0c_98f1;
const ID_EQ: u32 = 0x380c_ad68;

fn is_threshold_ladder(tokens: &[u32]) -> bool {
    let if_count = tokens.iter().filter(|&&t| t == ID_IF).count();
    let ret_count = tokens.iter().filter(|&&t| t == ID_RETURN).count();
    if_count >= 2 && ret_count >= 2 && tokens.len() <= 80
}

fn is_switch_table(tokens: &[u32]) -> bool {
    let case_count = tokens
        .iter()
        .filter(|&&t| t == ID_CASE || t == ID_ARROW)
        .count();
    let switch_or_match = tokens.iter().any(|&t| t == ID_SWITCH || t == ID_MATCH);
    switch_or_match && case_count >= 2 && tokens.len() <= 80
}

fn is_builder_chain(tokens: &[u32]) -> bool {
    let dot_count = tokens.iter().filter(|&&t| t == ID_DOT).count();
    let eq_count = tokens.iter().filter(|&&t| t == ID_EQ).count();
    eq_count >= 3 && dot_count >= 3 && tokens.len() <= 80
}

fn classify_body_idiom(tokens: &[u32]) -> CloneIdiom {
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
    let first = classify_body_idiom(&first_member.norm_token_ids);
    if first == CloneIdiom::None {
        return CloneIdiom::None;
    }
    for m in &members[1..] {
        if classify_body_idiom(&m.norm_token_ids) != first {
            return CloneIdiom::None;
        }
    }
    first
}

fn extract_file_candidates(
    file_bytes: &[u8],
    syms: &[&crate::domain::Symbol],
    min_tokens: usize,
) -> Vec<BodyTokens> {
    let mut out = Vec::new();
    for sym in syms {
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
        let norm_token_ids: Vec<u32> = norm_tokens.iter().map(|t| token_id(t.as_bytes())).collect();
        let kgrams = compute_kgrams_ids(&norm_token_ids, 5);
        out.push(BodyTokens {
            id: sym.id,
            raw_hash,
            norm_hash,
            norm_token_ids,
            kgrams,
        });
    }
    out
}

fn extract_candidate_bodies(
    ingest: &IngestResult,
    interner: &Interner,
    min_tokens: usize,
    sources: Option<&HashMap<String, Vec<u8>>>,
    root: Option<&std::path::Path>,
) -> Vec<BodyTokens> {
    let mut symbols_by_file: Vec<Vec<&crate::domain::Symbol>> =
        vec![Vec::new(); ingest.files.len()];
    for sym in &ingest.symbols {
        if (sym.kind == SymKind::Function || sym.kind == SymKind::Method)
            && sym.end_byte > sym.sig_start_byte
        {
            let fid = sym.file_id as usize;
            if fid < symbols_by_file.len() {
                symbols_by_file[fid].push(sym);
            }
        }
    }

    symbols_by_file
        .into_par_iter()
        .enumerate()
        .filter(|(_, syms)| !syms.is_empty())
        .flat_map(|(fid, syms)| {
            let file_info = ingest.files.get(fid)?;
            let file_path = interner.resolve(&file_info.path);
            let file_bytes = if let Some(map) = sources {
                map.get(file_path).cloned()
            } else if let Some(r) = root {
                std::fs::read(r.join(file_path)).ok()
            } else {
                std::fs::read(file_path).ok()
            }?;
            Some(extract_file_candidates(&file_bytes, &syms, min_tokens))
        })
        .flatten()
        .collect()
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
        let token_count = list[0].norm_token_ids.len();
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

const TYPE3_MAX_BUCKET: usize = 1024;
const TYPE3_MAX_TOKENS_FOR_LCS: usize = 4096;

fn evaluate_candidate_pair(ca: &BodyTokens, cb: &BodyTokens) -> Option<CloneGroup> {
    let len_a = ca.norm_token_ids.len();
    let len_b = cb.norm_token_ids.len();
    let min_len = len_a.min(len_b) as f64;
    let max_len = len_a.max(len_b) as f64;
    if min_len / max_len < 0.70 {
        return None;
    }

    if compute_jaccard(&ca.kgrams, &cb.kgrams) < 0.40 {
        return None;
    }

    let end_a = ca.norm_token_ids.len().min(TYPE3_MAX_TOKENS_FOR_LCS);
    let end_b = cb.norm_token_ids.len().min(TYPE3_MAX_TOKENS_FOR_LCS);
    let a_ids = &ca.norm_token_ids[..end_a];
    let b_ids = &cb.norm_token_ids[..end_b];

    let lcs = compute_lcs_len(a_ids, b_ids);
    let similarity = (2.0 * lcs as f64) / (a_ids.len() + b_ids.len()) as f64;

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

fn build_kgram_inverted_index(
    candidates: &[BodyTokens],
    exact_matched: &HashMap<NodeId, ()>,
) -> HashMap<u64, Vec<u32>> {
    let mut index: HashMap<u64, Vec<u32>> = HashMap::new();
    for (idx, c) in candidates.iter().enumerate() {
        if exact_matched.contains_key(&c.id) {
            continue;
        }
        for &kg in &c.kgrams {
            index.entry(kg).or_default().push(idx as u32);
        }
    }
    index
}

fn collect_bucket_pairs(candidates: &[BodyTokens], bucket: &[u32], pairs: &mut Vec<u64>) {
    for i in 0..bucket.len() {
        let ia = bucket[i];
        let len_a = candidates[ia as usize].norm_token_ids.len();
        for &ib in &bucket[i + 1..] {
            let len_b = candidates[ib as usize].norm_token_ids.len();
            let min_len = len_a.min(len_b) as f64;
            let max_len = len_a.max(len_b) as f64;
            if min_len / max_len >= 0.70 {
                let (lo, hi) = if ia < ib { (ia, ib) } else { (ib, ia) };
                pairs.push(((lo as u64) << 32) | (hi as u64));
            }
        }
    }
}

fn collect_type3_candidate_pairs(
    candidates: &[BodyTokens],
    index: HashMap<u64, Vec<u32>>,
) -> Vec<u64> {
    let mut pairs = Vec::new();
    for bucket in index.into_values() {
        if bucket.len() >= 2 && bucket.len() <= TYPE3_MAX_BUCKET {
            collect_bucket_pairs(candidates, &bucket, &mut pairs);
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}

fn find_type3_pairs(
    candidates: &[BodyTokens],
    exact_matched: &HashMap<NodeId, ()>,
) -> Vec<CloneGroup> {
    let index = build_kgram_inverted_index(candidates, exact_matched);
    let pairs = collect_type3_candidate_pairs(candidates, index);

    let mut gapped: Vec<CloneGroup> = pairs
        .into_par_iter()
        .filter_map(|key| {
            let ia = (key >> 32) as usize;
            let ib = (key as u32) as usize;
            evaluate_candidate_pair(&candidates[ia], &candidates[ib])
        })
        .collect();

    gapped.sort_by(|a, b| {
        a.members
            .cmp(&b.members)
            .then_with(|| b.token_count.cmp(&a.token_count))
    });
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
    find_clones_in_root(ingest, interner, min_tokens, sources, None)
}

/// Detect all Type-1, Type-2, and Type-3 clones across the codebase in target root.
#[must_use]
pub fn find_clones_in_root(
    ingest: &IngestResult,
    interner: &Interner,
    min_tokens: usize,
    sources: Option<&HashMap<String, Vec<u8>>>,
    root: Option<&std::path::Path>,
) -> Vec<CloneGroup> {
    let mut candidates = extract_candidate_bodies(ingest, interner, min_tokens, sources, root);
    candidates.sort_unstable_by_key(|c| c.norm_token_ids.len());
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

    #[test]
    fn test_clone_type3_gapped_detection() {
        let interner = Interner::new();
        let registry = LanguageRegistry::new().unwrap();

        let code_base = b"fn process_items(items: &[i32]) -> i32 {\n    let mut total = 0;\n    for &item in items {\n        if item > 0 {\n            total += item * 2;\n        }\n    }\n    total\n}\n";
        let code_near = b"fn process_values(values: &[i32]) -> i32 {\n    let mut total = 0;\n    let bonus = 1;\n    for &elem in values {\n        if elem > 0 {\n            total += elem * 2;\n        }\n    }\n    total + bonus\n}\n";

        let mut sources = HashMap::new();
        sources.insert("src/base.rs".into(), code_base.to_vec());
        sources.insert("src/near.rs".into(), code_near.to_vec());

        let ingest = ingest_in_memory_files(
            &[
                ("src/base.rs", Lang::Rust, code_base),
                ("src/near.rs", Lang::Rust, code_near),
            ],
            &registry,
            &interner,
        );

        let clones = find_clones(&ingest, &interner, 10, Some(&sources));
        assert!(!clones.is_empty(), "Should detect Type-3 near-miss clone");
        let t3 = clones.iter().find(|g| g.clone_type == CloneType::Type3);
        assert!(t3.is_some(), "Expected Type-3 clone");
        let g = t3.unwrap();
        assert_eq!(g.members.len(), 2);
        assert!(g.similarity >= 0.80 && g.similarity < 1.0);
    }
}
