use crate::domain::NodeId;
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

pub(super) fn build_in_edges_csr(node_count: usize, edges: &[(NodeId, NodeId, f32)]) -> CsrMatrix {
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
