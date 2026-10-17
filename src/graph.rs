use crate::domain::{IngestResult, Interner, NodeId};
pub mod community;
pub mod mermaid;
pub mod zoom;
use crate::rank::{PageRankConfig, PageRankResult, page_rank, uniform_teleport};
use crate::resolve::{ResolutionResult, resolve_all_references};
pub use mermaid::generate_mermaid_diagram;
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// Compressed Sparse Row (CSR) matrix representation.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CsrMatrix {
    pub rows: usize,
    pub cols: usize,
    pub row_offsets: Vec<u32>,
    pub col_indices: Vec<u32>,
    pub values: Vec<f32>,
}

impl CsrMatrix {
    #[must_use]
    pub fn new(
        rows: usize,
        cols: usize,
        row_offsets: Vec<u32>,
        col_indices: Vec<u32>,
        values: Vec<f32>,
    ) -> Self {
        Self {
            rows,
            cols,
            row_offsets,
            col_indices,
            values,
        }
    }

    #[must_use]
    pub fn empty(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            row_offsets: vec![0; rows + 1],
            col_indices: Vec::new(),
            values: Vec::new(),
        }
    }

    #[must_use]
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn row_range(&self, row: usize) -> Range<usize> {
        let start = self.row_offsets[row] as usize;
        let end = self.row_offsets[row + 1] as usize;
        start..end
    }

    pub fn row_entries(&self, row: usize) -> impl Iterator<Item = (NodeId, f32)> + '_ {
        let range = self.row_range(row);
        range.map(move |i| (self.col_indices[i], self.values[i]))
    }

    fn verify_row(&self, start: usize, end: usize) -> bool {
        if start > end || end > self.col_indices.len() {
            return false;
        }
        let mut prev_col = None;
        for i in start..end {
            let col = self.col_indices[i];
            let val = self.values[i];
            if col as usize >= self.cols || !val.is_finite() || val < 0.0 {
                return false;
            }
            if prev_col.is_some_and(|prev| col <= prev) {
                return false;
            }
            prev_col = Some(col);
        }
        true
    }

    #[must_use]
    pub fn verify(&self) -> bool {
        if self.row_offsets.len() != self.rows + 1
            || self.row_offsets[0] != 0
            || self.row_offsets[self.rows] != self.nnz() as u32
            || self.col_indices.len() != self.values.len()
        {
            return false;
        }
        (0..self.rows).all(|r| {
            let start = self.row_offsets[r] as usize;
            let end = self.row_offsets[r + 1] as usize;
            self.verify_row(start, end)
        })
    }
}

/// The resolved repository call graph.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Graph {
    pub in_edges: CsrMatrix,
    pub w_out_deg: Vec<f64>,
    pub out_offsets: Vec<u32>,
    pub out_targets: Vec<NodeId>,
    pub out_weights: Vec<f32>,
    pub out_split: Vec<bool>,
    pub amb_out: Vec<u32>,
    #[serde(default)]
    pub unresolved: usize,
    #[serde(default)]
    pub unindexed: usize,
    #[serde(default)]
    pub prior_weights: Vec<f64>,
}

impl Graph {
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.in_edges.rows
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.in_edges.nnz()
    }

    #[must_use]
    pub fn is_dangling(&self, node: NodeId) -> bool {
        self.w_out_deg[node as usize] == 0.0
    }

    #[must_use]
    pub fn out_degree(&self, node: NodeId) -> usize {
        let n = node as usize;
        (self.out_offsets[n + 1] - self.out_offsets[n]) as usize
    }

    #[must_use]
    pub fn in_degree(&self, node: NodeId) -> usize {
        let n = node as usize;
        (self.in_edges.row_offsets[n + 1] - self.in_edges.row_offsets[n]) as usize
    }

    pub fn callers(&self, node: NodeId) -> impl Iterator<Item = (NodeId, f32)> + '_ {
        self.in_edges.row_entries(node as usize)
    }

    pub fn callees(&self, node: NodeId) -> impl Iterator<Item = (NodeId, f32)> + '_ {
        self.callees_with_prov(node).map(|(id, w, _)| (id, w))
    }
    pub fn callees_with_prov(
        &self,
        node: NodeId,
    ) -> impl Iterator<Item = (NodeId, f32, bool)> + '_ {
        let n = node as usize;
        let start = self.out_offsets[n] as usize;
        let end = self.out_offsets[n + 1] as usize;
        (start..end).map(move |i| {
            (
                self.out_targets[i],
                self.out_weights[i],
                self.out_split.get(i).copied().unwrap_or(false),
            )
        })
    }

    #[must_use]
    pub fn ambiguity(&self, node: NodeId) -> u32 {
        self.amb_out[node as usize]
    }

    #[must_use]
    pub fn total_ambiguous(&self) -> usize {
        self.amb_out.iter().map(|&x| x as usize).sum()
    }

    #[must_use]
    pub fn total_unresolved(&self) -> usize {
        self.unresolved
    }

    #[must_use]
    pub fn total_unindexed(&self) -> usize {
        self.unindexed
    }
    #[must_use]
    pub fn gauges(&self) -> crate::serialize::GraphGauges {
        crate::serialize::GraphGauges {
            ambiguous: self.total_ambiguous(),
            unresolved: self.total_unresolved(),
            unindexed: self.total_unindexed(),
        }
    }

    #[must_use]
    pub fn xml_gauge_attrs(&self) -> String {
        format!(
            "graph_ambiguous=\"{}\" graph_unresolved=\"{}\" graph_unindexed=\"{}\" counts_floor=\"1\"",
            self.total_ambiguous(),
            self.total_unresolved(),
            self.total_unindexed()
        )
    }

    #[must_use]
    pub fn page_rank(&self, config: &PageRankConfig) -> PageRankResult {
        let teleport =
            if self.prior_weights.len() == self.node_count() && !self.prior_weights.is_empty() {
                crate::rank::biased_teleport(&self.prior_weights)
            } else {
                uniform_teleport(self.node_count())
            };
        self.page_rank_custom(config, &teleport)
    }

    #[must_use]
    pub fn page_rank_custom(&self, config: &PageRankConfig, teleport: &[f64]) -> PageRankResult {
        page_rank(&self.in_edges, &self.w_out_deg, teleport, config)
    }

    #[must_use]
    pub fn communities(&self) -> community::Communities {
        community::detect_communities(self)
    }

    #[must_use]
    pub fn hits(&self) -> crate::rank::HitsResult {
        crate::rank::compute_hits(self, &crate::rank::HitsConfig::default())
    }

    #[must_use]
    pub fn build(ingest: &IngestResult, interner: &Interner) -> Self {
        let resolution = resolve_all_references(ingest, interner);
        let mut sym_files = vec![false; ingest.files.len()];
        for s in &ingest.symbols {
            if let Some(slot) = sym_files.get_mut(s.file_id as usize) {
                *slot = true;
            }
        }
        let unindexed = ingest
            .files
            .iter()
            .enumerate()
            .filter(|&(i, f)| {
                !f.lang.semantics().is_prose_document()
                    && !sym_files.get(i).copied().unwrap_or(false)
            })
            .count();
        let mut g = Self::from_resolution(ingest.symbols.len(), resolution);
        g.unindexed = unindexed;
        g.prior_weights = compute_prior_weights(ingest, interner);
        g
    }

    #[must_use]
    pub fn from_resolution(node_count: usize, resolution: ResolutionResult) -> Self {
        let mut edges: Vec<(NodeId, NodeId, f32, bool)> = resolution
            .edge_accumulators
            .into_iter()
            .map(|((from, to), acc)| {
                let w = (acc.conf_sum / acc.n_ref as f32) * (acc.n_ref as f32).sqrt();
                (from, to, w.min(8.0), acc.is_split)
            })
            .collect();

        edges.sort_by_key(|&(from, to, _, _)| (from, to));

        let out = build_out_edges(node_count, &edges);
        let in_edges_tuples: Vec<(NodeId, NodeId, f32)> =
            edges.iter().map(|&(f, t, w, _)| (f, t, w)).collect();
        let in_edges = build_in_edges_csr(node_count, &in_edges_tuples);

        Self {
            in_edges,
            w_out_deg: out.w_out_deg,
            out_offsets: out.offsets,
            out_targets: out.targets,
            out_weights: out.weights,
            out_split: out.split,
            amb_out: resolution.amb_out,
            unresolved: resolution.unresolved,
            unindexed: 0,
            prior_weights: Vec::new(),
        }
    }
}

fn identifier_word_count(id: &str) -> usize {
    let mut words = 0usize;
    let mut in_word = false;
    let mut prev = 0u8;
    for &b in id.as_bytes() {
        if !b.is_ascii_alphanumeric() {
            in_word = false;
            prev = b;
            continue;
        }
        let camel_boundary = b.is_ascii_uppercase() && in_word && !prev.is_ascii_uppercase();
        if !in_word || camel_boundary {
            words += 1;
        }
        in_word = true;
        prev = b;
    }
    words
}

fn compute_prior_weights(ingest: &IngestResult, interner: &Interner) -> Vec<f64> {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let mut name_counts: std::collections::HashMap<lasso::Spur, usize> =
        std::collections::HashMap::with_capacity(ingest.symbols.len());
    for s in &ingest.symbols {
        *name_counts.entry(s.name).or_default() += 1;
    }
    let has_prod = ingest.symbols.iter().any(|s| view.is_production_symbol(s));
    ingest
        .symbols
        .iter()
        .map(|s| {
            let name = view.sym_name(s);
            let def_cnt = name_counts.get(&s.name).copied().unwrap_or(0);
            let mut w = 1.0f64;
            if def_cnt > 5 {
                w *= 0.5;
            }
            if name.starts_with('_') {
                w *= 0.5;
            }
            if name.len() >= 8 && identifier_word_count(name) >= 2 {
                w *= 1.7;
            }
            if has_prod && !view.is_production_symbol(s) {
                w *= 0.15;
            }
            w
        })
        .collect()
}

struct OutEdges {
    offsets: Vec<u32>,
    targets: Vec<NodeId>,
    weights: Vec<f32>,
    split: Vec<bool>,
    w_out_deg: Vec<f64>,
}

fn build_out_edges(node_count: usize, edges: &[(NodeId, NodeId, f32, bool)]) -> OutEdges {
    let mut out_offsets = vec![0u32; node_count + 1];
    for &(from, _, _, _) in edges {
        out_offsets[from as usize + 1] += 1;
    }
    for i in 0..node_count {
        out_offsets[i + 1] += out_offsets[i];
    }

    let mut out_targets = vec![0u32; edges.len()];
    let mut out_weights = vec![0.0f32; edges.len()];
    let mut out_split = vec![false; edges.len()];
    let mut w_out_deg = vec![0.0f64; node_count];
    let mut cur = out_offsets[0..node_count].to_vec();

    for &(from, to, w, is_split) in edges {
        let pos = cur[from as usize] as usize;
        cur[from as usize] += 1;
        out_targets[pos] = to;
        out_weights[pos] = w;
        out_split[pos] = is_split;
        w_out_deg[from as usize] += w as f64;
    }

    OutEdges {
        offsets: out_offsets,
        targets: out_targets,
        weights: out_weights,
        split: out_split,
        w_out_deg,
    }
}
fn build_in_edges_csr(node_count: usize, edges: &[(NodeId, NodeId, f32)]) -> CsrMatrix {
    let mut row_offsets = vec![0u32; node_count + 1];
    for &(_, to, _) in edges {
        row_offsets[to as usize + 1] += 1;
    }
    for i in 0..node_count {
        row_offsets[i + 1] += row_offsets[i];
    }

    let mut col_indices = vec![0u32; edges.len()];
    let mut values = vec![0.0f32; edges.len()];
    let mut cur = row_offsets[0..node_count].to_vec();

    for &(from, to, w) in edges {
        let pos = cur[to as usize] as usize;
        cur[to as usize] += 1;
        col_indices[pos] = from;
        values[pos] = w;
    }

    for t in 0..node_count {
        let start = row_offsets[t] as usize;
        let end = row_offsets[t + 1] as usize;
        if end > start + 1 {
            let mut pairs: Vec<(u32, f32)> =
                (start..end).map(|i| (col_indices[i], values[i])).collect();
            pairs.sort_by_key(|&(col, _)| col);
            for (idx, (col, val)) in pairs.into_iter().enumerate() {
                col_indices[start + idx] = col;
                values[start + idx] = val;
            }
        }
    }

    CsrMatrix::new(node_count, node_count, row_offsets, col_indices, values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csr_matrix_empty_and_verify() {
        let empty = CsrMatrix::empty(4, 4);
        assert_eq!(empty.rows, 4);
        assert_eq!(empty.cols, 4);
        assert_eq!(empty.nnz(), 0);
        assert!(empty.verify());
    }

    #[test]
    fn test_graph_simple_chain() {
        let mut resolution = ResolutionResult {
            amb_out: vec![0, 0, 0],
            ..Default::default()
        };
        resolution.edge_accumulators.insert(
            (0, 1),
            crate::resolve::EdgeAcc {
                conf_sum: 1.0,
                n_ref: 1,
                is_split: false,
            },
        );
        resolution.edge_accumulators.insert(
            (1, 2),
            crate::resolve::EdgeAcc {
                conf_sum: 1.0,
                n_ref: 1,
                is_split: false,
            },
        );

        let graph = Graph::from_resolution(3, resolution);
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
        assert!(graph.in_edges.verify());

        assert_eq!(graph.out_degree(0), 1);
        assert_eq!(graph.out_degree(1), 1);
        assert_eq!(graph.out_degree(2), 0);
        assert!(graph.is_dangling(2));
        assert!(!graph.is_dangling(0));

        assert_eq!(graph.in_degree(0), 0);
        assert_eq!(graph.in_degree(1), 1);
        assert_eq!(graph.in_degree(2), 1);

        let callers_of_2: Vec<_> = graph.callers(2).collect();
        assert_eq!(callers_of_2, vec![(1, 1.0)]);

        let callees_of_0: Vec<_> = graph.callees(0).collect();
        assert_eq!(callees_of_0, vec![(1, 1.0)]);
    }
}
