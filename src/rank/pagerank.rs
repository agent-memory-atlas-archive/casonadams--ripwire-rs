use crate::graph::CsrMatrix;

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
#[derive(Debug, Clone, PartialEq)]
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

/// Scale current rank by weighted out-degree. Dangling nodes scale to 0.0.
pub fn scale_ranks_by_out_degree(current_rank: &[f64], w_out_deg: &[f64], scaled: &mut [f64]) {
    for i in 0..current_rank.len() {
        scaled[i] = if w_out_deg[i] > 0.0 {
            current_rank[i] / w_out_deg[i]
        } else {
            0.0
        };
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
    for target in 0..in_edges.rows {
        let mut incoming = 0.0f64;
        for (source, weight) in in_edges.row_entries(target) {
            incoming += (weight as f64) * scaled[source as usize];
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

    let mut current_rank = teleport.to_vec();
    let mut next_rank = vec![0.0f64; n];
    let mut scaled_rank = vec![0.0f64; n];
    let mut converged = false;
    let mut iters = 0;

    for iter in 0..config.max_iterations {
        let dangling_mass = compute_dangling_mass(&current_rank, w_out_deg);
        scale_ranks_by_out_degree(&current_rank, w_out_deg, &mut scaled_rank);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform_and_biased_teleport() {
        let u = uniform_teleport(4);
        assert_eq!(u.len(), 4);
        assert!((reduce_block_sum(&u) - 1.0).abs() < 1e-9);

        let biased = biased_teleport(&[1.0, 2.0, 1.0]);
        assert_eq!(biased.len(), 3);
        assert!((biased[0] - 0.25).abs() < 1e-9);
        assert!((biased[1] - 0.50).abs() < 1e-9);
        assert!((biased[2] - 0.25).abs() < 1e-9);

        let empty = uniform_teleport(0);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_empty_graph_pagerank() {
        let empty_csr = CsrMatrix::empty(0, 0);
        let res = page_rank(&empty_csr, &[], &[], &PageRankConfig::default());
        assert_eq!(res.iterations, 0);
        assert!(res.converged);
        assert!(res.ranks.is_empty());
    }

    #[test]
    fn test_truncated_iterations() {
        let csr = CsrMatrix::new(2, 2, vec![0, 0, 1], vec![0], vec![1.0]);
        let w_out = vec![1.0, 0.0];
        let teleport = uniform_teleport(2);
        let cfg = PageRankConfig {
            max_iterations: 1,
            tolerance: 1e-12,
            alpha: 0.85,
        };
        let res = page_rank(&csr, &w_out, &teleport, &cfg);
        assert_eq!(res.iterations, 1);
        assert!(!res.converged);
    }
}
