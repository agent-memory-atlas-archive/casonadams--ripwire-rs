use super::token_stream::{
    SKETCH_HASHES, SKETCH_MIN_MATCHES, compute_jaccard, compute_kgrams_ids, compute_lcs_len,
    compute_sketch, scan_body_ids, sketch_matches,
};
use crate::domain::{IngestResult, Interner, NodeId, SymKind};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

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
    sketch: [u8; SKETCH_HASHES],
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

fn symbol_body_range(sym: &crate::domain::Symbol, file_len: usize) -> (usize, usize) {
    (
        (sym.sig_end_byte as usize).min(file_len),
        (sym.end_byte as usize).min(file_len),
    )
}

fn extract_file_candidates(
    file_bytes: &[u8],
    syms: &[&crate::domain::Symbol],
    min_tokens: usize,
) -> Vec<BodyTokens> {
    let mut out = Vec::with_capacity(syms.len());
    for sym in syms {
        let (start, end) = symbol_body_range(sym, file_bytes.len());
        if start >= end {
            continue;
        }
        let (raw_hash, norm_hash, norm_token_ids) =
            scan_body_ids(&file_bytes[start..end], sym.lang);
        if norm_token_ids.len() < min_tokens {
            continue;
        }
        let kgrams = compute_kgrams_ids(&norm_token_ids, 5);
        let sketch = compute_sketch(&kgrams);
        out.push(BodyTokens {
            id: sym.id,
            raw_hash,
            norm_hash,
            norm_token_ids,
            kgrams,
            sketch,
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
            && sym.end_byte > sym.sig_end_byte
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
    groups.sort_by(|a, b| {
        b.token_count
            .cmp(&a.token_count)
            .then_with(|| a.members.cmp(&b.members))
    });
    groups
}

const TYPE3_MAX_BUCKET: usize = 1024;
const TYPE3_MAX_TOKENS_FOR_LCS: usize = 4096;
const TYPE3_MAX_PAIRS: usize = 200_000;

fn evaluate_candidate_pair(ca: &BodyTokens, cb: &BodyTokens) -> Option<CloneGroup> {
    if compute_jaccard(&ca.kgrams, &cb.kgrams) < 0.40 {
        return None;
    }
    let len_a = ca.norm_token_ids.len();
    let len_b = cb.norm_token_ids.len();
    let a_ids = &ca.norm_token_ids[..len_a.min(TYPE3_MAX_TOKENS_FOR_LCS)];
    let b_ids = &cb.norm_token_ids[..len_b.min(TYPE3_MAX_TOKENS_FOR_LCS)];
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

fn build_dedup_kgram_index(candidates: &[BodyTokens]) -> HashMap<u64, Vec<u32>> {
    let mut index: HashMap<u64, Vec<u32>> = HashMap::new();
    let mut seen_streams: HashSet<u64> = HashSet::with_capacity(candidates.len());
    for (idx, c) in candidates.iter().enumerate() {
        if !seen_streams.insert(c.norm_hash) {
            continue;
        }
        for &kg in &c.kgrams {
            index.entry(kg).or_default().push(idx as u32);
        }
    }
    index
}

fn try_stage_pair(
    candidates: &[BodyTokens],
    ia: u32,
    ib: u32,
    seen: &mut HashSet<u64>,
    survivors: &mut Vec<u64>,
) {
    let (lo, hi) = if ia < ib { (ia, ib) } else { (ib, ia) };
    let len_a = candidates[lo as usize].norm_token_ids.len();
    let len_b = candidates[hi as usize].norm_token_ids.len();
    if len_a.min(len_b) * 100 < len_a.max(len_b) * 70 {
        return;
    }
    let key = ((lo as u64) << 32) | (hi as u64);
    if !seen.insert(key) {
        return;
    }
    if sketch_matches(
        &candidates[lo as usize].sketch,
        &candidates[hi as usize].sketch,
    ) >= SKETCH_MIN_MATCHES
    {
        survivors.push(key);
    }
}

fn collect_type3_survivor_pairs(
    candidates: &[BodyTokens],
    index: HashMap<u64, Vec<u32>>,
) -> Vec<u64> {
    let mut seen: HashSet<u64> = HashSet::new();
    let mut survivors = Vec::new();
    for bucket in index.into_values() {
        if bucket.len() < 2 || bucket.len() > TYPE3_MAX_BUCKET {
            continue;
        }
        for (i, &ia) in bucket.iter().enumerate() {
            for &ib in &bucket[i + 1..] {
                if survivors.len() >= TYPE3_MAX_PAIRS {
                    return survivors;
                }
                try_stage_pair(candidates, ia, ib, &mut seen, &mut survivors);
            }
        }
    }
    survivors
}

fn find_type3_pairs(candidates: &[BodyTokens]) -> Vec<CloneGroup> {
    let index = build_dedup_kgram_index(candidates);
    let pairs = collect_type3_survivor_pairs(candidates, index);

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
    candidates.sort_unstable_by_key(|c| c.id);
    let mut exact_groups = group_exact_clones(&candidates);
    let gapped_groups = find_type3_pairs(&candidates);
    exact_groups.extend(gapped_groups);
    exact_groups
}

fn find_root(parent: &mut HashMap<NodeId, NodeId>, mut x: NodeId) -> NodeId {
    while parent[&x] != x {
        let p = parent[&x];
        let gp = parent[&p];
        parent.insert(x, gp);
        x = p;
    }
    x
}

fn union_group_members(parent: &mut HashMap<NodeId, NodeId>, members: &[NodeId]) {
    let Some(&first) = members.first() else {
        return;
    };
    let r0 = find_root(parent, first);
    for &m in &members[1..] {
        let rm = find_root(parent, m);
        if r0 != rm {
            parent.insert(rm, r0);
        }
    }
}

fn assign_component_gids(groups: &[CloneGroup]) -> (Vec<usize>, usize) {
    let mut parent: HashMap<NodeId, NodeId> = HashMap::new();
    for g in groups {
        for &m in &g.members {
            parent.entry(m).or_insert(m);
        }
    }
    for g in groups {
        union_group_members(&mut parent, &g.members);
    }
    let mut comp_id_map: HashMap<NodeId, usize> = HashMap::new();
    let mut gids = Vec::with_capacity(groups.len());
    for g in groups {
        let gid = if let Some(&first) = g.members.first() {
            let root = find_root(&mut parent, first);
            let next_id = comp_id_map.len() + 1;
            *comp_id_map.entry(root).or_insert(next_id)
        } else {
            0
        };
        gids.push(gid);
    }
    (gids, comp_id_map.len())
}

fn compute_clone_components(
    groups: &[CloneGroup],
    ingest: &IngestResult,
) -> (Vec<usize>, usize, usize, usize) {
    let (gids, comp_count) = assign_component_gids(groups);
    let mut comp_members: HashMap<usize, HashSet<NodeId>> = HashMap::new();
    for (g, &gid) in groups.iter().zip(&gids) {
        comp_members.entry(gid).or_default().extend(&g.members);
    }
    let mut dup_loc = 0usize;
    for members in comp_members.values() {
        let mut sum_loc = 0usize;
        let mut max_loc = 0usize;
        for &m in members {
            let loc = ingest.symbols.get(m as usize).map_or(0, |s| s.loc as usize);
            sum_loc += loc;
            max_loc = max_loc.max(loc);
        }
        dup_loc += sum_loc.saturating_sub(max_loc);
    }
    let total_loc: usize = ingest
        .symbols
        .iter()
        .filter(|s| {
            (s.kind == SymKind::Function || s.kind == SymKind::Method)
                && s.end_byte > s.sig_end_byte
        })
        .map(|s| s.loc as usize)
        .sum();
    (gids, comp_count, dup_loc, total_loc)
}

fn select_windowed_clone_indices(groups: &[CloneGroup], cap: usize) -> (usize, usize, Vec<usize>) {
    let mut exact_idx = Vec::new();
    let mut type3_idx = Vec::new();
    for (idx, g) in groups.iter().enumerate() {
        if g.clone_type == CloneType::Type3 {
            type3_idx.push(idx);
        } else {
            exact_idx.push(idx);
        }
    }
    let keep12 = exact_idx.len().min(cap);
    let keep3 = type3_idx.len().min(cap);
    let mut shown_indices = Vec::with_capacity(keep12 + keep3);
    shown_indices.extend_from_slice(&exact_idx[..keep12]);
    shown_indices.extend_from_slice(&type3_idx[..keep3]);
    (exact_idx.len(), type3_idx.len(), shown_indices)
}

fn format_clone_group_xml(
    g: &CloneGroup,
    gid: usize,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut String,
) {
    let idiom_attr = match g.idiom.as_str() {
        Some(name) => format!(" idiom=\"{name}\""),
        None => String::new(),
    };
    if g.clone_type == CloneType::Type3 {
        out.push_str(&format!(
            "  <group type=\"3\" gid=\"{gid}\" tokens=\"{}\" n=\"{}\" similarity=\"{:.2}\"{idiom_attr}>\n",
            g.token_count,
            g.members.len(),
            g.similarity,
        ));
    } else {
        out.push_str(&format!(
            "  <group type=\"2\" gid=\"{gid}\" tokens=\"{}\" n=\"{}\"{idiom_attr}>\n",
            g.token_count,
            g.members.len(),
        ));
    }
    for &id in &g.members {
        if let Some(sym) = ingest.symbols.get(id as usize) {
            use crate::serialize::XmlEscapeExt;
            let name = interner.resolve(&sym.name).xml_escaped();
            let p = if let Some(fi) = ingest.files.get(sym.file_id as usize) {
                format!("{}:{}", interner.resolve(&fi.path).xml_escaped(), sym.line)
            } else {
                format!("line:{}", sym.line)
            };
            out.push_str(&format!("    <f n=\"{name}\" p=\"{p}\"/>\n"));
        }
    }
    out.push_str("  </group>\n");
}

/// Format clone detection results as Ripwire XML.
#[must_use]
pub fn render_clones_xml(
    groups: &[CloneGroup],
    ingest: &IngestResult,
    interner: &Interner,
) -> String {
    let (gids, clone_groups, dup_loc, total_loc) = compute_clone_components(groups, ingest);
    let (exact_count, type3_count, shown_indices) = select_windowed_clone_indices(groups, 40);
    let total = exact_count + type3_count;
    let shown = shown_indices.len();
    let capped = usize::from(shown < total);
    let idiom_groups = groups
        .iter()
        .filter(|g| g.idiom != CloneIdiom::None)
        .count();
    let dup_pct = if total_loc > 0 {
        (dup_loc as f64 * 100.0) / (total_loc as f64)
    } else {
        0.0
    };
    let page_attrs = format!(
        " shown=\"{shown}\" capped=\"{capped}\" total=\"{total}\" has_more=\"{capped}\" next_offset=\"{shown}\" offset=\"0\" limit=\"0\""
    );

    let mut out = format!(
        "<!-- ripwire clones schema=ripwire.clones/v1: similar normalized-token bodies: <group type=2|3 gid= tokens= n= similarity=> of <f n= p=>; dup_loc=/dup_pct=. -->\n<clones schema=\"ripwire.clones/v1\" groups=\"{exact_count}\" type3=\"{type3_count}\" exempt_groups=\"0\" idiom_groups=\"{idiom_groups}\" demoted_groups=\"{idiom_groups}\" clone_groups=\"{clone_groups}\" dup_loc=\"{dup_loc}\" total_loc=\"{total_loc}\" dup_pct=\"{dup_pct:.1}\"{page_attrs} root=\".\">\n"
    );

    for idx in shown_indices {
        format_clone_group_xml(&groups[idx], gids[idx], ingest, interner, &mut out);
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
