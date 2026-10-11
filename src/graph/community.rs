use super::Graph;
use crate::domain::NodeId;
use std::collections::HashMap;

/// Result of Louvain community detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Communities {
    /// Community ID for each node (0..node_count-1), compacted to 0..count-1 in first-seen order.
    pub comm: Vec<u32>,
    /// Number of distinct non-empty communities.
    pub count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct UndirectedEdge {
    target: NodeId,
    weight: f32,
}

fn build_undirected_adj(graph: &Graph) -> (Vec<Vec<UndirectedEdge>>, f64) {
    let n = graph.node_count();
    let mut edge_map: Vec<HashMap<NodeId, f32>> = vec![HashMap::new(); n];

    for u in 0..n as NodeId {
        for (v, w) in graph.callees(u) {
            if u != v {
                *edge_map[u as usize].entry(v).or_default() += w;
                *edge_map[v as usize].entry(u).or_default() += w;
            }
        }
    }

    let mut adj = vec![Vec::new(); n];
    let mut m2 = 0.0f64;

    for (u, map) in edge_map.into_iter().enumerate() {
        let mut edges: Vec<UndirectedEdge> = map
            .into_iter()
            .map(|(target, weight)| UndirectedEdge { target, weight })
            .collect();
        edges.sort_by_key(|e| e.target);
        for e in &edges {
            m2 += f64::from(e.weight);
        }
        adj[u] = edges;
    }

    (adj, m2)
}

fn find_best_community(
    node_deg: f64,
    link_to: &HashMap<u32, f64>,
    comm_tot: &[f64],
    m2: f64,
    current_comm: u32,
) -> u32 {
    let mut best = current_comm;
    let base_gain = link_to.get(&current_comm).copied().unwrap_or(0.0)
        - (node_deg * comm_tot[current_comm as usize] / m2);
    let mut best_gain = base_gain;

    let mut candidate_comms: Vec<(u32, f64)> = link_to.iter().map(|(&c, &k)| (c, k)).collect();
    candidate_comms.sort_by_key(|&(c, _)| c);

    for (c, kin) in candidate_comms {
        let gain = kin - (node_deg * comm_tot[c as usize] / m2);
        if gain > best_gain + 1e-10 || ((gain - best_gain).abs() <= 1e-10 && c < best) {
            best_gain = gain;
            best = c;
        }
    }

    best
}

fn perform_louvain_pass(
    adj: &[Vec<UndirectedEdge>],
    deg: &[f64],
    m2: f64,
    comm: &mut [u32],
    comm_tot: &mut [f64],
) -> bool {
    let mut improved = false;

    for (i, edges) in adj.iter().enumerate() {
        if deg[i] == 0.0 {
            continue;
        }

        let ci = comm[i];
        comm_tot[ci as usize] -= deg[i];

        let mut link_to = HashMap::new();
        for e in edges {
            let neighbor_comm = comm[e.target as usize];
            *link_to.entry(neighbor_comm).or_insert(0.0f64) += f64::from(e.weight);
        }

        let best = find_best_community(deg[i], &link_to, comm_tot, m2, ci);
        comm_tot[best as usize] += deg[i];
        comm[i] = best;

        if best != ci {
            improved = true;
        }
    }

    improved
}

fn compact_community_ids(comm: &[u32]) -> (Vec<u32>, u32) {
    let mut remap: HashMap<u32, u32> = HashMap::new();
    let mut next_id = 0u32;
    let mut compacted = Vec::with_capacity(comm.len());

    for &c in comm {
        let id = *remap.entry(c).or_insert_with(|| {
            let id = next_id;
            next_id += 1;
            id
        });
        compacted.push(id);
    }

    (compacted, next_id)
}

/// Detect cohesive communities in the call graph using the Louvain modularity algorithm.
#[must_use]
pub fn detect_communities(graph: &Graph) -> Communities {
    let n = graph.node_count();
    if n == 0 {
        return Communities {
            comm: Vec::new(),
            count: 0,
        };
    }

    let (adj, m2) = build_undirected_adj(graph);
    if m2 <= 0.0 {
        return Communities {
            comm: vec![0; n],
            count: 1,
        };
    }

    let deg: Vec<f64> = adj
        .iter()
        .map(|edges| edges.iter().map(|e| f64::from(e.weight)).sum())
        .collect();

    let mut comm: Vec<u32> = (0..n as u32).collect();
    let mut comm_tot = deg.clone();

    for _pass in 0..16 {
        if !perform_louvain_pass(&adj, &deg, m2, &mut comm, &mut comm_tot) {
            break;
        }
    }

    let (compacted, count) = compact_community_ids(&comm);
    Communities {
        comm: compacted,
        count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Interner;
    use crate::ingest::LanguageRegistry;

    #[test]
    fn test_community_detection_two_cliques() {
        let interner = Interner::new();
        let registry = LanguageRegistry::new().unwrap();

        // Two distinct clusters: (a, b, c) and (x, y, z)
        let code1 = b"fn a() { b(); c(); } fn b() { a(); c(); } fn c() { a(); b(); }";
        let code2 = b"fn x() { y(); z(); } fn y() { x(); z(); } fn z() { x(); y(); }";

        let ingest = crate::ingest::ingest_in_memory_files(
            &[
                ("src/cluster1.rs", crate::domain::Lang::Rust, code1),
                ("src/cluster2.rs", crate::domain::Lang::Rust, code2),
            ],
            &registry,
            &interner,
        );

        let graph = Graph::build(&ingest, &interner);
        let comms = detect_communities(&graph);

        assert_eq!(comms.comm.len(), 6);
        assert_eq!(comms.count, 2);
        // First cluster nodes share community
        assert_eq!(comms.comm[0], comms.comm[1]);
        assert_eq!(comms.comm[1], comms.comm[2]);
        // Second cluster nodes share community
        assert_eq!(comms.comm[3], comms.comm[4]);
        assert_eq!(comms.comm[4], comms.comm[5]);
        // Distinct clusters have different community IDs
        assert_ne!(comms.comm[0], comms.comm[3]);
    }
}
