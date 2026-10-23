use super::token_stream::{
    SKETCH_HASHES, SKETCH_MIN_MATCHES, compute_jaccard, compute_kgrams_ids, compute_lcs_len,
    compute_sketch, scan_body_ids, sketch_matches,
};
use crate::domain::{IngestResult, Interner, NodeId, SymKind};
use crate::serialize::{PageRequest, PageWindow};
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

fn count_token(tokens: &[u32], id: u32) -> usize {
    tokens.iter().filter(|&&t| t == id).count()
}

fn classify_body_idiom(tokens: &[u32]) -> CloneIdiom {
    if tokens.len() > 80 {
        return CloneIdiom::None;
    }
    if count_token(tokens, ID_IF) >= 2 && count_token(tokens, ID_RETURN) >= 2 {
        CloneIdiom::ThresholdLadder
    } else if tokens.iter().any(|&t| t == ID_SWITCH || t == ID_MATCH)
        && (count_token(tokens, ID_CASE) + count_token(tokens, ID_ARROW)) >= 2
    {
        CloneIdiom::SwitchNameTable
    } else if count_token(tokens, ID_EQ) >= 3 && count_token(tokens, ID_DOT) >= 3 {
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

fn load_clone_file_bytes(
    file_path: &str,
    sources: Option<&HashMap<String, Vec<u8>>>,
    root: Option<&std::path::Path>,
) -> Option<Vec<u8>> {
    if let Some(map) = sources {
        map.get(file_path).cloned()
    } else if let Some(r) = root {
        crate::domain::read_rel_bytes(r, file_path).ok()
    } else {
        std::fs::read(file_path).ok()
    }
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
            && let Some(bucket) = symbols_by_file.get_mut(sym.file_id as usize)
        {
            bucket.push(sym);
        }
    }

    symbols_by_file
        .into_par_iter()
        .enumerate()
        .filter(|(_, syms)| !syms.is_empty())
        .filter_map(|(fid, syms)| {
            let fi = ingest.files.get(fid)?;
            let bytes = load_clone_file_bytes(interner.resolve(&fi.path), sources, root)?;
            Some(extract_file_candidates(&bytes, &syms, min_tokens))
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
/// Bodies shorter than this need a stricter Type-3 floor: fluent chains and loops of a few calls share
/// enough normalized tokens by chance to reach 0.80 between unrelated functions.
const TYPE3_SHORT_BODY_TOKENS: usize = 60;
const TYPE3_FLOOR: f64 = 0.80;
const TYPE3_SHORT_BODY_FLOOR: f64 = 0.85;
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

    let floor = if len_a.min(len_b) < TYPE3_SHORT_BODY_TOKENS {
        TYPE3_SHORT_BODY_FLOOR
    } else {
        TYPE3_FLOOR
    };
    if (floor..1.0).contains(&similarity) {
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

fn select_windowed_clone_indices(
    groups: &[CloneGroup],
    gids: &[usize],
    ingest: &IngestResult,
    interner: &Interner,
    page: PageRequest,
) -> (usize, usize, Vec<String>, PageWindow) {
    let (mut order, type3_idx): (Vec<usize>, Vec<usize>) =
        (0..groups.len()).partition(|&i| groups[i].clone_type != CloneType::Type3);
    let (exact, type3) = (order.len(), type3_idx.len());
    order.extend(type3_idx);
    let rows: Vec<String> = order
        .iter()
        .map(|&idx| {
            let mut row = String::new();
            format_clone_group_xml(&groups[idx], gids[idx], ingest, interner, &mut row);
            row
        })
        .collect();
    let window = crate::serialize::Paged::new(&rows, page, |row| row.len()).window;
    let shown = rows
        .into_iter()
        .skip(window.begin)
        .take(window.shown)
        .collect();
    (exact, type3, shown, window)
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
    use crate::domain::CorpusView;
    use crate::serialize::XmlEscapeExt;
    let view = (ingest, interner);
    for &id in &g.members {
        if let Some(sym) = ingest.sym(id) {
            let name = view.sym_name(sym).xml_escaped();
            let fp = view.sym_path(sym);
            let p = if !fp.is_empty() {
                format!("{}:{}", fp.xml_escaped(), sym.line)
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
    page: PageRequest,
) -> String {
    let (gids, clone_groups, dup_loc, total_loc) = compute_clone_components(groups, ingest);
    let (exact_count, type3_count, shown_rows, window) =
        select_windowed_clone_indices(groups, &gids, ingest, interner, page);
    let idiom_groups = groups
        .iter()
        .filter(|g| g.idiom != CloneIdiom::None)
        .count();
    let dup_pct = if total_loc > 0 {
        (dup_loc as f64 * 100.0) / (total_loc as f64)
    } else {
        0.0
    };
    let page_attrs = window.xml_attrs();

    let mut out = format!(
        "<!-- ripwire-rs clones schema=ripwire-rs.clones/v1: similar normalized-token bodies: <group type=2|3 gid= tokens= n= similarity=> of <f n= p=>; dup_loc=/dup_pct=. -->\n<clones schema=\"ripwire-rs.clones/v1\" groups=\"{exact_count}\" type3=\"{type3_count}\" idiom_groups=\"{idiom_groups}\" clone_groups=\"{clone_groups}\" dup_loc=\"{dup_loc}\" total_loc=\"{total_loc}\" dup_pct=\"{dup_pct:.1}\"{page_attrs} root=\".\">\n"
    );

    out.extend(shown_rows);

    out.push_str("</clones>\n");
    out
}
