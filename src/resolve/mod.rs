pub mod ladder;

pub use ladder::*;

use crate::domain::{IngestResult, Interner, NodeId, Reference, Symbol};
use lasso::Spur;
use rayon::prelude::*;
use std::collections::HashMap;

/// Intermediate structure accumulating confidence and reference counts per directed edge.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct EdgeAcc {
    pub conf_sum: f32,
    pub n_ref: u32,
    pub is_split: bool,
}

/// Resolved graph edges and symbol ambiguity counts.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ResolutionResult {
    pub edge_accumulators: HashMap<(NodeId, NodeId), EdgeAcc>,
    pub amb_out: Vec<u32>,
}

/// Resolve all call references from an ingestion result into edge accumulators and ambiguity metrics.
#[must_use]
pub fn resolve_reference_chunk(
    chunk: &[Reference],
    symbols: &[Symbol],
    file_dirs: &[u32],
    by_name: &HashMap<Spur, Vec<NodeId>>,
    num_symbols: usize,
) -> (HashMap<(NodeId, NodeId), EdgeAcc>, Vec<u32>) {
    let mut edge_accumulators: HashMap<(NodeId, NodeId), EdgeAcc> = HashMap::new();
    let mut amb_out = vec![0u32; num_symbols];

    for r in chunk {
        if let Some(resolved) = resolve_call_reference(r, symbols, file_dirs, by_name) {
            if resolved.is_ambiguous {
                amb_out[resolved.from as usize] += 1;
            }
            for to in resolved.targets {
                let acc = edge_accumulators.entry((resolved.from, to)).or_default();
                acc.conf_sum += resolved.conf_per_target;
                acc.n_ref += 1;
                acc.is_split |= resolved.is_ambiguous;
            }
        }
    }

    (edge_accumulators, amb_out)
}

pub fn merge_resolutions(
    mut a: (HashMap<(NodeId, NodeId), EdgeAcc>, Vec<u32>),
    b: (HashMap<(NodeId, NodeId), EdgeAcc>, Vec<u32>),
) -> (HashMap<(NodeId, NodeId), EdgeAcc>, Vec<u32>) {
    for (k, acc_b) in b.0 {
        let acc_a = a.0.entry(k).or_default();
        acc_a.conf_sum += acc_b.conf_sum;
        acc_a.n_ref += acc_b.n_ref;
        acc_a.is_split |= acc_b.is_split;
    }
    for (i, val) in b.1.into_iter().enumerate() {
        a.1[i] += val;
    }
    a
}

pub fn resolve_all_references(ingest: &IngestResult, interner: &Interner) -> ResolutionResult {
    let file_dirs = compute_file_dirs(&ingest.files, interner);
    let by_name = index_symbols_by_name(&ingest.symbols);
    let num_symbols = ingest.symbols.len();

    let (edge_accumulators, amb_out) = if ingest.references.len() < 256 {
        resolve_reference_chunk(
            &ingest.references,
            &ingest.symbols,
            &file_dirs,
            &by_name,
            num_symbols,
        )
    } else {
        ingest
            .references
            .par_chunks(512)
            .map(|chunk| {
                resolve_reference_chunk(chunk, &ingest.symbols, &file_dirs, &by_name, num_symbols)
            })
            .reduce(
                || (HashMap::new(), vec![0u32; num_symbols]),
                merge_resolutions,
            )
    };

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
