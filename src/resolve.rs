use crate::domain::{
    FileInfo, IngestResult, Interner, NodeId, RefRole, Reference, Symbol, NO_NODE,
};
use lasso::Spur;
use std::collections::HashMap;
use std::path::Path;

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
fn select_precedence_tier(
    cands: &[NodeId],
    caller_file_id: u32,
    caller_dir_id: u32,
    symbols: &[Symbol],
    file_dirs: &[u32],
) -> Option<(Vec<NodeId>, f32)> {
    let same_file: Vec<NodeId> = cands
        .iter()
        .copied()
        .filter(|&id| symbols[id as usize].file_id == caller_file_id)
        .collect();
    if !same_file.is_empty() {
        return Some((same_file, 1.0));
    }

    let same_dir: Vec<NodeId> = cands
        .iter()
        .copied()
        .filter(|&id| {
            let fid = symbols[id as usize].file_id as usize;
            file_dirs.get(fid).copied() == Some(caller_dir_id)
        })
        .collect();
    if !same_dir.is_empty() {
        return Some((same_dir, 0.5));
    }

    if cands.len() == 1 {
        Some((cands.to_vec(), 0.2))
    } else {
        None
    }
}

/// Resolve a single reference according to the precedence ladder.
#[must_use]
pub fn resolve_call_reference(
    r: &Reference,
    symbols: &[Symbol],
    file_dirs: &[u32],
    by_name: &HashMap<Spur, Vec<NodeId>>,
) -> Option<ResolvedRef> {
    if r.role != RefRole::Call || r.from_symbol == NO_NODE {
        return None;
    }
    let all_cands = by_name.get(&r.callee_name)?;
    let mut cands: Vec<NodeId> = all_cands
        .iter()
        .copied()
        .filter(|&id| symbols[id as usize].lang.is_compatible(r.lang))
        .collect();
    if cands.is_empty() {
        return None;
    }

    if let Some(q) = r.qualifier {
        let qualified: Vec<NodeId> = cands
            .iter()
            .copied()
            .filter(|&id| symbols[id as usize].scope == Some(q))
            .collect();
        if !qualified.is_empty() {
            cands = qualified;
        }
    }

    let caller_file_id = r.file_id;
    let caller_dir_id = file_dirs
        .get(caller_file_id as usize)
        .copied()
        .unwrap_or(u32::MAX);
    let (tier, conf) =
        select_precedence_tier(&cands, caller_file_id, caller_dir_id, symbols, file_dirs)?;

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

/// Intermediate structure accumulating confidence and reference counts per directed edge.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct EdgeAcc {
    pub conf_sum: f32,
    pub n_ref: u32,
}

/// Resolved graph edges and symbol ambiguity counts.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ResolutionResult {
    pub edge_accumulators: HashMap<(NodeId, NodeId), EdgeAcc>,
    pub amb_out: Vec<u32>,
}

/// Resolve all call references from an ingestion result into edge accumulators and ambiguity metrics.
#[must_use]
pub fn resolve_all_references(ingest: &IngestResult, interner: &Interner) -> ResolutionResult {
    let file_dirs = compute_file_dirs(&ingest.files, interner);
    let by_name = index_symbols_by_name(&ingest.symbols);
    let mut edge_accumulators: HashMap<(NodeId, NodeId), EdgeAcc> = HashMap::new();
    let mut amb_out = vec![0u32; ingest.symbols.len()];

    for r in &ingest.references {
        if let Some(resolved) = resolve_call_reference(r, &ingest.symbols, &file_dirs, &by_name) {
            if resolved.is_ambiguous {
                amb_out[resolved.from as usize] += 1;
            }
            for to in resolved.targets {
                let acc = edge_accumulators.entry((resolved.from, to)).or_default();
                acc.conf_sum += resolved.conf_per_target;
                acc.n_ref += 1;
            }
        }
    }

    ResolutionResult {
        edge_accumulators,
        amb_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        FileInfo, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol,
    };

    #[test]
    fn test_precedence_ladder_same_file() {
        let interner = Interner::new();
        let f0 = FileInfo {
            id: 0,
            path: interner.intern("src/a.rs"),
            lang: Lang::Rust,
            line_count: 10,
            byte_size: 100,
        };
        let f1 = FileInfo {
            id: 1,
            path: interner.intern("src/b.rs"),
            lang: Lang::Rust,
            line_count: 10,
            byte_size: 100,
        };
        let name = interner.intern("target");
        let canon = interner.intern("canon");
        let caller = Symbol::new(
            0,
            interner.intern("caller"),
            canon,
            SymKind::Function,
            Lang::Rust,
            0,
            1,
        );
        let cand_same_file = Symbol::new(1, name, canon, SymKind::Function, Lang::Rust, 0, 5);
        let cand_other_file = Symbol::new(2, name, canon, SymKind::Function, Lang::Rust, 1, 5);

        let call = Reference::new(0, name, 0, 2, Lang::Rust, RefRole::Call);

        let mut ingest = IngestResult::new();
        ingest.files.push(f0);
        ingest.files.push(f1);
        ingest.symbols.push(caller);
        ingest.symbols.push(cand_same_file);
        ingest.symbols.push(cand_other_file);
        ingest.references.push(call);

        let res = resolve_all_references(&ingest, &interner);
        assert_eq!(res.edge_accumulators.len(), 1);
        assert!(res.edge_accumulators.contains_key(&(0, 1)));
        assert_eq!(res.edge_accumulators[&(0, 1)].conf_sum, 1.0);
        assert_eq!(res.edge_accumulators[&(0, 1)].n_ref, 1);
        assert_eq!(res.amb_out[0], 0);
    }

    #[test]
    fn test_precedence_ladder_same_dir_and_unique_global() {
        let interner = Interner::new();
        let f0 = FileInfo {
            id: 0,
            path: interner.intern("pkg/a.rs"),
            lang: Lang::Rust,
            line_count: 10,
            byte_size: 100,
        };
        let f1 = FileInfo {
            id: 1,
            path: interner.intern("pkg/b.rs"),
            lang: Lang::Rust,
            line_count: 10,
            byte_size: 100,
        };
        let f2 = FileInfo {
            id: 2,
            path: interner.intern("other/c.rs"),
            lang: Lang::Rust,
            line_count: 10,
            byte_size: 100,
        };

        let name = interner.intern("shared");
        let canon = interner.intern("canon");
        let caller = Symbol::new(
            0,
            interner.intern("caller"),
            canon,
            SymKind::Function,
            Lang::Rust,
            0,
            1,
        );
        let cand_same_dir = Symbol::new(1, name, canon, SymKind::Function, Lang::Rust, 1, 5);
        let cand_other_dir = Symbol::new(2, name, canon, SymKind::Function, Lang::Rust, 2, 5);

        let call = Reference::new(0, name, 0, 2, Lang::Rust, RefRole::Call);

        let mut ingest = IngestResult::new();
        ingest.files.extend_from_slice(&[f0, f1, f2]);
        ingest
            .symbols
            .extend_from_slice(&[caller, cand_same_dir, cand_other_dir]);
        ingest.references.push(call);

        let res = resolve_all_references(&ingest, &interner);
        assert_eq!(res.edge_accumulators.len(), 1);
        assert!(res.edge_accumulators.contains_key(&(0, 1)));
        assert_eq!(res.edge_accumulators[&(0, 1)].conf_sum, 0.5);

        // Test unique global
        let cand_unique_global = Symbol::new(1, name, canon, SymKind::Function, Lang::Rust, 2, 5);
        let mut ingest_global = IngestResult::new();
        ingest_global.files.extend_from_slice(&[f0, f1, f2]);
        ingest_global
            .symbols
            .extend_from_slice(&[caller, cand_unique_global]);
        ingest_global.references.push(call);

        let res_global = resolve_all_references(&ingest_global, &interner);
        assert_eq!(res_global.edge_accumulators.len(), 1);
        assert!(res_global.edge_accumulators.contains_key(&(0, 1)));
        assert_eq!(res_global.edge_accumulators[&(0, 1)].conf_sum, 0.2);
    }

    #[test]
    fn test_precedence_ladder_non_unique_global_dropped() {
        let interner = Interner::new();
        let f0 = FileInfo {
            id: 0,
            path: interner.intern("src/a.rs"),
            lang: Lang::Rust,
            line_count: 10,
            byte_size: 100,
        };
        let f1 = FileInfo {
            id: 1,
            path: interner.intern("dir1/b.rs"),
            lang: Lang::Rust,
            line_count: 10,
            byte_size: 100,
        };
        let f2 = FileInfo {
            id: 2,
            path: interner.intern("dir2/c.rs"),
            lang: Lang::Rust,
            line_count: 10,
            byte_size: 100,
        };

        let name = interner.intern("common");
        let canon = interner.intern("canon");
        let caller = Symbol::new(
            0,
            interner.intern("caller"),
            canon,
            SymKind::Function,
            Lang::Rust,
            0,
            1,
        );
        let cand1 = Symbol::new(1, name, canon, SymKind::Function, Lang::Rust, 1, 5);
        let cand2 = Symbol::new(2, name, canon, SymKind::Function, Lang::Rust, 2, 5);
        let call = Reference::new(0, name, 0, 2, Lang::Rust, RefRole::Call);

        let mut ingest = IngestResult::new();
        ingest.files.extend_from_slice(&[f0, f1, f2]);
        ingest.symbols.extend_from_slice(&[caller, cand1, cand2]);
        ingest.references.push(call);

        let res = resolve_all_references(&ingest, &interner);
        assert!(res.edge_accumulators.is_empty());
    }

    #[test]
    fn test_discard_self_loops() {
        let interner = Interner::new();
        let f0 = FileInfo {
            id: 0,
            path: interner.intern("src/a.rs"),
            lang: Lang::Rust,
            line_count: 10,
            byte_size: 100,
        };
        let name = interner.intern("recursive");
        let canon = interner.intern("canon");
        let sym = Symbol::new(0, name, canon, SymKind::Function, Lang::Rust, 0, 1);
        let call = Reference::new(0, name, 0, 2, Lang::Rust, RefRole::Call);

        let mut ingest = IngestResult::new();
        ingest.files.push(f0);
        ingest.symbols.push(sym);
        ingest.references.push(call);

        let res = resolve_all_references(&ingest, &interner);
        assert!(res.edge_accumulators.is_empty());
        assert_eq!(res.amb_out[0], 0);
    }
}
