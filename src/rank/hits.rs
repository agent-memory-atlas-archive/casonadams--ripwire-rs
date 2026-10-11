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

fn compute_authority_step(graph: &Graph, hub: &[f64], authority: &mut [f64]) {
    for (i, auth_val) in authority.iter_mut().enumerate() {
        let mut sum = 0.0;
        for (src, w) in graph.callers(i as u32) {
            sum += (w as f64) * hub[src as usize];
        }
        *auth_val = sum;
    }
    normalize_vector_l2(authority);
}

fn compute_hub_step(graph: &Graph, authority: &[f64], hub: &mut [f64]) {
    for (j, hub_val) in hub.iter_mut().enumerate() {
        let mut sum = 0.0;
        for (tgt, w) in graph.callees(j as u32) {
            sum += (w as f64) * authority[tgt as usize];
        }
        *hub_val = sum;
    }
    normalize_vector_l2(hub);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{IngestResult, Interner, Lang, SymKind, Symbol};

    #[test]
    fn test_hits_empty_graph() {
        let interner = Interner::new();
        let ingest = IngestResult::new();
        let graph = Graph::build(&ingest, &interner);
        let res = compute_hits(&graph, &HitsConfig::default());
        assert_eq!(res.authority.len(), 0);
        assert_eq!(res.hub.len(), 0);
        assert!(res.converged);
    }

    #[test]
    fn test_hits_two_node_chain() {
        let interner = Interner::new();
        let mut ingest = IngestResult::new();
        let f = interner.intern("test.rs");
        let n0 = interner.intern("caller");
        let n1 = interner.intern("callee");

        ingest
            .symbols
            .push(Symbol::new(0, f, n0, SymKind::Function, Lang::Rust, 0, 1));
        ingest
            .symbols
            .push(Symbol::new(1, f, n1, SymKind::Function, Lang::Rust, 0, 10));

        let resolution = crate::resolve::ResolutionResult {
            edge_accumulators: std::collections::HashMap::from([(
                (0, 1),
                crate::resolve::EdgeAcc {
                    n_ref: 1,
                    conf_sum: 1.0,
                },
            )]),
            amb_out: vec![0, 0],
        };
        let graph = Graph::from_resolution(2, resolution);
        let res = compute_hits(&graph, &HitsConfig::default());

        assert_eq!(res.authority.len(), 2);
        assert_eq!(res.hub.len(), 2);
        assert!(res.converged);
        // Node 1 is callee -> high authority
        assert!(res.authority[1] > res.authority[0]);
        // Node 0 is caller -> high hub
        assert!(res.hub[0] > res.hub[1]);
    }
}
