use crate::domain::{FileInfo, Interner, NO_NODE, NodeId, RefRole, Reference, Symbol};
use lasso::Spur;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CandBuffer {
    inline: [NodeId; 8],
    len: usize,
    heap: Vec<NodeId>,
}

impl CandBuffer {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            inline: [0; 8],
            len: 0,
            heap: Vec::new(),
        }
    }

    #[inline]
    pub fn push(&mut self, id: NodeId) {
        if self.heap.is_empty() && self.len < 8 {
            self.inline[self.len] = id;
            self.len += 1;
        } else {
            if self.heap.is_empty() {
                self.heap.reserve(self.len + 8);
                self.heap.extend_from_slice(&self.inline[..self.len]);
            }
            self.heap.push(id);
        }
    }

    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[NodeId] {
        if self.heap.is_empty() {
            &self.inline[..self.len]
        } else {
            &self.heap
        }
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        if self.heap.is_empty() {
            self.len == 0
        } else {
            self.heap.is_empty()
        }
    }
}

impl Default for CandBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolved reference outcome with target nodes and per-target confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRef {
    pub from: NodeId,
    pub targets: Vec<NodeId>,
    pub conf_per_target: f32,
    pub is_ambiguous: bool,
}

/// Assign integer directory IDs based on the parent directory of each file.
#[must_use]
pub fn compute_file_dirs(files: &[FileInfo], interner: &Interner) -> Vec<u32> {
    let mut dir_map: HashMap<String, u32> = HashMap::new();
    let mut file_dirs = Vec::with_capacity(files.len());
    for f in files {
        let path = interner.resolve(&f.path);
        let dir = Path::new(path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string();
        let next_id = dir_map.len() as u32;
        let dir_id = *dir_map.entry(dir).or_insert(next_id);
        file_dirs.push(dir_id);
    }
    file_dirs
}

/// Index symbols by their interned name.
#[must_use]
pub fn index_symbols_by_name(symbols: &[Symbol]) -> HashMap<Spur, Vec<NodeId>> {
    let mut by_name: HashMap<Spur, Vec<NodeId>> = HashMap::new();
    for s in symbols {
        by_name.entry(s.name).or_default().push(s.id);
    }
    by_name
}

/// Select candidates via the precedence ladder:
/// Tier 1: Same file (conf = 1.0)
/// Tier 2: Same directory (conf = 0.5)
/// Tier 3: Unique same-language global (conf = 0.2), else dropped.
pub fn select_precedence_tier(
    cands: &[NodeId],
    caller_file_id: u32,
    caller_dir_id: u32,
    symbols: &[Symbol],
    file_dirs: &[u32],
) -> Option<(Vec<NodeId>, f32)> {
    if cands
        .iter()
        .any(|&id| symbols[id as usize].file_id == caller_file_id)
    {
        let same_file: Vec<NodeId> = cands
            .iter()
            .copied()
            .filter(|&id| symbols[id as usize].file_id == caller_file_id)
            .collect();
        return Some((same_file, 1.0));
    }

    if cands.iter().any(|&id| {
        let fid = symbols[id as usize].file_id as usize;
        file_dirs.get(fid).copied() == Some(caller_dir_id)
    }) {
        let same_dir: Vec<NodeId> = cands
            .iter()
            .copied()
            .filter(|&id| {
                let fid = symbols[id as usize].file_id as usize;
                file_dirs.get(fid).copied() == Some(caller_dir_id)
            })
            .collect();
        return Some((same_dir, 0.5));
    }

    if let Some(&first) = cands.first() {
        let first_file = symbols[first as usize].file_id;
        if cands
            .iter()
            .all(|&id| symbols[id as usize].file_id == first_file)
        {
            return Some((cands.to_vec(), 0.2));
        }
    }
    None
}

/// Resolve a single reference according to the precedence ladder.
#[must_use]
fn is_candidate_compatible(sym: &Symbol, r: &Reference) -> bool {
    if sym.lang == crate::domain::Lang::Sql && matches!(r.role, RefRole::Read | RefRole::Write) {
        return true;
    }
    sym.lang.is_compatible(r.lang) && (!r.arg_count_known || r.arg_count <= sym.params)
}

pub fn filter_candidates(
    all_cands: &[NodeId],
    r: &Reference,
    symbols: &[Symbol],
) -> Option<CandBuffer> {
    let mut cands = CandBuffer::default();
    for &id in all_cands {
        if is_candidate_compatible(&symbols[id as usize], r) {
            cands.push(id);
        }
    }
    if cands.is_empty() {
        return None;
    }

    if let Some(q) = r.qualifier {
        let mut qualified = CandBuffer::default();
        for &id in cands.as_slice() {
            if symbols[id as usize].scope == Some(q) {
                qualified.push(id);
            }
        }
        if !qualified.is_empty() {
            cands = qualified;
        }
    }

    Some(cands)
}

pub fn resolve_call_reference(
    r: &Reference,
    symbols: &[Symbol],
    file_dirs: &[u32],
    by_name: &HashMap<Spur, Vec<NodeId>>,
) -> Option<ResolvedRef> {
    if r.from_symbol == NO_NODE || !matches!(r.role, RefRole::Call | RefRole::Read | RefRole::Write)
    {
        return None;
    }
    let all_cands = by_name.get(&r.callee_name)?;
    let cands = filter_candidates(all_cands, r, symbols)?;

    let caller_file_id = r.file_id;
    let caller_dir_id = file_dirs
        .get(caller_file_id as usize)
        .copied()
        .unwrap_or(u32::MAX);
    let (tier, conf) = select_precedence_tier(
        cands.as_slice(),
        caller_file_id,
        caller_dir_id,
        symbols,
        file_dirs,
    )?;

    let real_targets: Vec<NodeId> = tier
        .into_iter()
        .filter(|&target| target != r.from_symbol)
        .collect();
    if real_targets.is_empty() {
        return None;
    }

    let is_ambiguous = real_targets.len() > 1;
    let conf_per_target = conf / (real_targets.len() as f32);
    Some(ResolvedRef {
        from: r.from_symbol,
        targets: real_targets,
        conf_per_target,
        is_ambiguous,
    })
}
