use crate::graph::CsrMatrix;
use serde::{Deserialize, Serialize};

/// Reduction block size for platform-independent deterministic float summation.
pub const REDUCTION_BLOCK_SIZE: usize = 1024;

/// Configuration for Personalized PageRank.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageRankConfig {
    pub alpha: f64,
    pub tolerance: f64,
    pub max_iterations: u32,
}

impl Default for PageRankConfig {
    fn default() -> Self {
        Self {
            alpha: 0.85,
            tolerance: 1e-6,
            max_iterations: 100,
        }
    }
}

/// Result of PageRank power iteration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageRankResult {
    pub iterations: u32,
    pub converged: bool,
    pub ranks: Vec<f64>,
}

/// Sum a slice of floats deterministically using contiguous 1024-element block reductions.
#[must_use]
pub fn reduce_block_sum(values: &[f64]) -> f64 {
    let mut total = 0.0f64;
    for block in values.chunks(REDUCTION_BLOCK_SIZE) {
        let mut partial = 0.0f64;
        for &v in block {
            partial += v;
        }
        total += partial;
    }
    total
}

/// Accumulate dangling mass from nodes with zero out-degree using 1024-block reductions.
#[must_use]
pub fn compute_dangling_mass(current_rank: &[f64], w_out_deg: &[f64]) -> f64 {
    let n = current_rank.len();
    let mut total = 0.0f64;
    for block_start in (0..n).step_by(REDUCTION_BLOCK_SIZE) {
        let block_end = (block_start + REDUCTION_BLOCK_SIZE).min(n);
        let mut partial = 0.0f64;
        for i in block_start..block_end {
            if w_out_deg[i] <= 0.0 {
                partial += current_rank[i];
            }
        }
        total += partial;
    }
    total
}

/// Compute L1 residual between successive iteration vectors using 1024-block reductions.
#[must_use]
pub fn compute_l1_residual(next: &[f64], current: &[f64]) -> f64 {
    let n = next.len();
    let mut total = 0.0f64;
    for block_start in (0..n).step_by(REDUCTION_BLOCK_SIZE) {
        let block_end = (block_start + REDUCTION_BLOCK_SIZE).min(n);
        let mut partial = 0.0f64;
        for i in block_start..block_end {
            partial += (next[i] - current[i]).abs();
        }
        total += partial;
    }
    total
}

/// Precompute reciprocal out-degrees so power iteration multiplies instead of dividing.
#[must_use]
#[inline]
pub fn compute_inv_out_degrees(w_out_deg: &[f64]) -> Vec<f64> {
    w_out_deg
        .iter()
        .map(|&w| if w > 0.0 { 1.0 / w } else { 0.0 })
        .collect()
}

/// Scale current rank by precomputed reciprocal out-degrees.
#[inline]
pub fn scale_ranks_by_inv_out_degree(
    current_rank: &[f64],
    inv_out_deg: &[f64],
    scaled: &mut [f64],
) {
    for i in 0..current_rank.len() {
        scaled[i] = current_rank[i] * inv_out_deg[i];
    }
}

/// Gather rank along incoming CSR edges and add redistributed mass.
pub fn gather_in_edges(
    in_edges: &CsrMatrix,
    scaled: &[f64],
    alpha: f64,
    teleport_scale: f64,
    teleport: &[f64],
    next: &mut [f64],
) {
    let row_offsets = &in_edges.row_offsets;
    let col_indices = &in_edges.col_indices;
    let values = &in_edges.values;

    for target in 0..in_edges.rows {
        let start = row_offsets[target] as usize;
        let end = row_offsets[target + 1] as usize;
        let mut incoming = 0.0f64;
        for i in start..end {
            let source = col_indices[i] as usize;
            let weight = values[i] as f64;
            incoming += weight * scaled[source];
        }
        next[target] = alpha * incoming + teleport_scale * teleport[target];
    }
}

/// Generate a uniform teleport prior vector summing to 1.0.
#[must_use]
pub fn uniform_teleport(n: usize) -> Vec<f64> {
    if n == 0 {
        Vec::new()
    } else {
        vec![1.0 / (n as f64); n]
    }
}

/// Generate a normalized teleport vector from custom weights.
#[must_use]
pub fn biased_teleport(weights: &[f64]) -> Vec<f64> {
    let sum = reduce_block_sum(weights);
    if sum <= 0.0 || !sum.is_finite() {
        return uniform_teleport(weights.len());
    }
    weights.iter().map(|&w| w / sum).collect()
}

/// Execute deterministic Personalized PageRank with dangling mass redistribution.
#[must_use]
pub fn page_rank(
    in_edges: &CsrMatrix,
    w_out_deg: &[f64],
    teleport: &[f64],
    config: &PageRankConfig,
) -> PageRankResult {
    let n = in_edges.rows;
    if n == 0 {
        return PageRankResult {
            iterations: 0,
            converged: true,
            ranks: Vec::new(),
        };
    }

    let inv_out_deg = compute_inv_out_degrees(w_out_deg);
    let mut current_rank = teleport.to_vec();
    let mut next_rank = vec![0.0f64; n];
    let mut scaled_rank = vec![0.0f64; n];
    let mut converged = false;
    let mut iters = 0;

    for iter in 0..config.max_iterations {
        let dangling_mass = compute_dangling_mass(&current_rank, w_out_deg);
        scale_ranks_by_inv_out_degree(&current_rank, &inv_out_deg, &mut scaled_rank);
        let teleport_scale = config.alpha * dangling_mass + (1.0 - config.alpha);
        gather_in_edges(
            in_edges,
            &scaled_rank,
            config.alpha,
            teleport_scale,
            teleport,
            &mut next_rank,
        );
        let residual = compute_l1_residual(&next_rank, &current_rank);
        current_rank.copy_from_slice(&next_rank);
        iters = iter + 1;
        if residual < config.tolerance {
            converged = true;
            break;
        }
    }

    PageRankResult {
        iterations: iters,
        converged,
        ranks: current_rank,
    }
}
