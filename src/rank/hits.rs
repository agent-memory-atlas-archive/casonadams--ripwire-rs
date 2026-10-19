use crate::graph::Graph;

/// Configuration for Kleinberg HITS algorithm.
#[derive(Debug, Clone, PartialEq)]
pub struct HitsConfig {
    pub tol: f64,
    pub max_iter: usize,
}

impl Default for HitsConfig {
    fn default() -> Self {
        Self {
            tol: 1e-6,
            max_iter: 100,
        }
    }
}

/// Result of Kleinberg HITS calculation holding Authority and Hub vectors.
#[derive(Debug, Clone, PartialEq)]
pub struct HitsResult {
    pub authority: Vec<f64>,
    pub hub: Vec<f64>,
    pub iterations: usize,
    pub converged: bool,
}

fn compute_l2_norm(vec: &[f64]) -> f64 {
    let sum_sq: f64 = vec.iter().map(|&v| v * v).sum();
    sum_sq.sqrt()
}

fn normalize_vector_l2(vec: &mut [f64]) {
    let norm = compute_l2_norm(vec);
    if norm > 0.0 {
        let inv_norm = 1.0 / norm;
        for v in vec.iter_mut() {
            *v *= inv_norm;
        }
    }
}

fn compute_csr_step(
    offsets: &[u32],
    targets: &[u32],
    weights: &[f32],
    src: &[f64],
    dst: &mut [f64],
) {
    for (i, dst_val) in dst.iter_mut().enumerate() {
        let start = offsets[i] as usize;
        let end = offsets[i + 1] as usize;
        *dst_val = (start..end).fold(0.0, |acc, k| {
            acc + (weights[k] as f64) * src[targets[k] as usize]
        });
    }
    normalize_vector_l2(dst);
}

fn compute_authority_step(graph: &Graph, hub: &[f64], authority: &mut [f64]) {
    let in_edges = &graph.in_edges;
    compute_csr_step(
        &in_edges.row_offsets,
        &in_edges.col_indices,
        &in_edges.values,
        hub,
        authority,
    );
}

fn compute_hub_step(graph: &Graph, authority: &[f64], hub: &mut [f64]) {
    compute_csr_step(
        &graph.out_offsets,
        &graph.out_targets,
        &graph.out_weights,
        authority,
        hub,
    );
}

fn compute_hits_residual(a: &[f64], a_prev: &[f64], h: &[f64], h_prev: &[f64]) -> f64 {
    let mut diff = 0.0;
    for i in 0..a.len() {
        diff += (a[i] - a_prev[i]).abs() + (h[i] - h_prev[i]).abs();
    }
    diff
}

/// Compute Kleinberg HITS (Hubs & Authorities) over the call graph.
pub fn compute_hits(graph: &Graph, config: &HitsConfig) -> HitsResult {
    let n = graph.node_count();
    if n == 0 {
        return HitsResult {
            authority: Vec::new(),
            hub: Vec::new(),
            iterations: 0,
            converged: true,
        };
    }

    let seed = 1.0 / (n as f64).sqrt();
    let mut authority = vec![seed; n];
    let mut hub = vec![seed; n];
    let mut a_prev = vec![0.0; n];
    let mut h_prev = vec![0.0; n];

    let mut iterations = 0;
    let mut converged = false;

    for it in 0..config.max_iter {
        iterations = it + 1;
        a_prev.copy_from_slice(&authority);
        h_prev.copy_from_slice(&hub);

        compute_authority_step(graph, &hub, &mut authority);
        compute_hub_step(graph, &authority, &mut hub);

        let residual = compute_hits_residual(&authority, &a_prev, &hub, &h_prev);
        if residual < config.tol {
            converged = true;
            break;
        }
    }

    HitsResult {
        authority,
        hub,
        iterations,
        converged,
    }
}
