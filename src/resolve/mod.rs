pub mod ladder;

pub use ladder::*;

use crate::domain::{IngestResult, Interner, NodeId, Reference};
use lasso::Spur;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

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
    pub unresolved: usize,
    pub external: usize,
    pub declined: usize,
}

/// Resolve all call references from an ingestion result into edge accumulators and ambiguity metrics.
fn record_resolved_ref(
    resolved: ResolvedRef,
    edge_accumulators: &mut HashMap<(NodeId, NodeId), EdgeAcc>,
    amb_out: &mut [u32],
) {
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

#[must_use]
pub fn resolve_reference_chunk_with_context<L: ResolutionLookup>(
    chunk: &[Reference],
    lookup: &L,
    num_symbols: usize,
) -> ResolutionResult {
    let mut res = ResolutionResult {
        edge_accumulators: HashMap::new(),
        amb_out: vec![0u32; num_symbols],
        unresolved: 0,
        external: 0,
        declined: 0,
    };
    for r in chunk {
        match lookup.classify_ref(r) {
            RefOutcome::Resolved(resolved) => {
                record_resolved_ref(resolved, &mut res.edge_accumulators, &mut res.amb_out);
            }
            RefOutcome::Unresolved => res.unresolved += 1,
            RefOutcome::External => res.external += 1,
            RefOutcome::Declined | RefOutcome::Builtin => res.declined += 1,
            RefOutcome::Ignored => {}
        }
    }
    res
}

#[must_use]
pub fn merge_resolutions(mut a: ResolutionResult, b: ResolutionResult) -> ResolutionResult {
    for (k, acc_b) in b.edge_accumulators {
        let acc_a = a.edge_accumulators.entry(k).or_default();
        acc_a.conf_sum += acc_b.conf_sum;
        acc_a.n_ref += acc_b.n_ref;
        acc_a.is_split |= acc_b.is_split;
    }
    for (i, val) in b.amb_out.into_iter().enumerate() {
        a.amb_out[i] += val;
    }
    a.unresolved += b.unresolved;
    a.external += b.external;
    a.declined += b.declined;
    a
}

pub struct ResolutionIndex {
    file_dirs: Vec<u32>,
    by_name: HashMap<Spur, Vec<NodeId>>,
    file_scopes: Vec<HashSet<Spur>>,
    module_qualifiers: Vec<HashMap<Spur, u32>>,
}

impl ResolutionIndex {
    #[must_use]
    pub fn build(ingest: &IngestResult, interner: &Interner) -> Self {
        Self {
            file_dirs: compute_file_dirs(&ingest.files, interner),
            by_name: index_symbols_by_name(&ingest.symbols),
            file_scopes: index_file_scope_mentions(ingest.files.len(), &ingest.references),
            module_qualifiers: index_module_qualifiers(ingest, interner),
        }
    }

    #[must_use]
    pub fn context<'a>(
        &'a self,
        ingest: &'a IngestResult,
        interner: &'a Interner,
    ) -> ResolutionContext<'a> {
        ResolutionContext {
            symbols: &ingest.symbols,
            file_dirs: &self.file_dirs,
            by_name: &self.by_name,
            interner: Some(interner),
            file_scopes: Some(&self.file_scopes),
            module_qualifiers: Some(&self.module_qualifiers),
        }
    }
}

pub fn resolve_all_references(ingest: &IngestResult, interner: &Interner) -> ResolutionResult {
    let index = ResolutionIndex::build(ingest, interner);
    let ctx = index.context(ingest, interner);
    let num_symbols = ingest.symbols.len();

    if ingest.references.len() < 256 {
        resolve_reference_chunk_with_context(&ingest.references, &ctx, num_symbols)
    } else {
        ingest
            .references
            .par_chunks(512)
            .map(|chunk| resolve_reference_chunk_with_context(chunk, &ctx, num_symbols))
            .reduce(
                || ResolutionResult {
                    edge_accumulators: HashMap::new(),
                    amb_out: vec![0u32; num_symbols],
                    unresolved: 0,
                    external: 0,
                    declined: 0,
                },
                merge_resolutions,
            )
    }
}
