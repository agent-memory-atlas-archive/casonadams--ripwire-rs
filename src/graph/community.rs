use super::Graph;
use crate::domain::NodeId;

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

fn coalesce_sorted_edges(edges: &mut Vec<UndirectedEdge>) -> f64 {
    if edges.is_empty() {
        return 0.0;
    }
    edges.sort_unstable_by_key(|e| e.target);
    let mut write_idx = 0usize;
    let mut m2 = f64::from(edges[0].weight);
    for read_idx in 1..edges.len() {
        let e = edges[read_idx];
        m2 += f64::from(e.weight);
        if edges[write_idx].target == e.target {
            edges[write_idx].weight += e.weight;
        } else {
            write_idx += 1;
            edges[write_idx] = e;
        }
    }
    edges.truncate(write_idx + 1);
    m2
}

fn build_undirected_adj(graph: &Graph) -> (Vec<Vec<UndirectedEdge>>, f64) {
    let n = graph.node_count();
    let mut adj: Vec<Vec<UndirectedEdge>> = vec![Vec::new(); n];

    for u in 0..n as NodeId {
        for (v, w) in graph.callees(u) {
            if u != v {
                adj[u as usize].push(UndirectedEdge {
                    target: v,
                    weight: w,
                });
                adj[v as usize].push(UndirectedEdge {
                    target: u,
                    weight: w,
                });
            }
        }
    }

    let mut m2 = 0.0f64;
    for edges in &mut adj {
        m2 += coalesce_sorted_edges(edges);
    }

    (adj, m2)
}

fn coalesce_comm_weights(scratch: &mut Vec<(u32, f64)>) {
    if scratch.len() <= 1 {
        return;
    }
    scratch.sort_unstable_by_key(|&(c, _)| c);
    let mut w = 0usize;
    for r in 1..scratch.len() {
        let (c, wt) = scratch[r];
        if scratch[w].0 == c {
            scratch[w].1 += wt;
        } else {
            w += 1;
            scratch[w] = (c, wt);
        }
    }
    scratch.truncate(w + 1);
}

fn find_best_community(
    node_deg: f64,
    candidate_comms: &[(u32, f64)],
    comm_tot: &[f64],
    m2: f64,
    current_comm: u32,
) -> u32 {
    let mut best = current_comm;
    let cur_kin = candidate_comms
        .iter()
        .find(|&&(c, _)| c == current_comm)
        .map_or(0.0, |&(_, k)| k);
    let mut best_gain = cur_kin - (node_deg * comm_tot[current_comm as usize] / m2);

    for &(c, kin) in candidate_comms {
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
    let mut scratch = Vec::with_capacity(32);

    for (i, edges) in adj.iter().enumerate() {
        if deg[i] == 0.0 {
            continue;
        }

        let ci = comm[i];
        comm_tot[ci as usize] -= deg[i];

        scratch.clear();
        for e in edges {
            scratch.push((comm[e.target as usize], f64::from(e.weight)));
        }
        coalesce_comm_weights(&mut scratch);

        let best = find_best_community(deg[i], &scratch, comm_tot, m2, ci);
        comm_tot[best as usize] += deg[i];
        comm[i] = best;

        if best != ci {
            improved = true;
        }
    }

    improved
}

fn compact_community_ids(comm: &[u32]) -> (Vec<u32>, u32) {
    let mut remap = vec![u32::MAX; comm.len()];
    let mut next_id = 0u32;
    let mut compacted = Vec::with_capacity(comm.len());

    for &c in comm {
        let slot = &mut remap[c as usize];
        if *slot == u32::MAX {
            *slot = next_id;
            next_id += 1;
        }
        compacted.push(*slot);
    }

    (compacted, next_id)
}

fn louvain_partition(adj: &[Vec<UndirectedEdge>], self_w: &[f64], m2: f64) -> (Vec<u32>, u32) {
    let deg: Vec<f64> = adj
        .iter()
        .zip(self_w)
        .map(|(edges, s)| edges.iter().map(|e| f64::from(e.weight)).sum::<f64>() + s)
        .collect();
    let mut comm: Vec<u32> = (0..adj.len() as u32).collect();
    let mut comm_tot = deg.clone();
    for _pass in 0..16 {
        if !perform_louvain_pass(adj, &deg, m2, &mut comm, &mut comm_tot) {
            break;
        }
    }
    compact_community_ids(&comm)
}

fn aggregate_communities(
    adj: &[Vec<UndirectedEdge>],
    self_w: &[f64],
    comm: &[u32],
    count: u32,
) -> (Vec<Vec<UndirectedEdge>>, Vec<f64>) {
    let mut next_adj: Vec<Vec<UndirectedEdge>> = vec![Vec::new(); count as usize];
    let mut next_self = vec![0.0f64; count as usize];
    for (u, edges) in adj.iter().enumerate() {
        let cu = comm[u] as usize;
        next_self[cu] += self_w[u];
        for e in edges {
            let cv = comm[e.target as usize];
            if cv as usize == cu {
                next_self[cu] += f64::from(e.weight);
            } else {
                next_adj[cu].push(UndirectedEdge {
                    target: cv,
                    weight: e.weight,
                });
            }
        }
    }
    for edges in &mut next_adj {
        coalesce_sorted_edges(edges);
    }
    (next_adj, next_self)
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
    let (comm, count) = louvain_partition(&adj, &vec![0.0; n], m2);
    Communities { comm, count }
}

/// Multi-level Louvain: each level aggregates the previous level's communities and re-runs local
/// moving. Returns one per-node assignment per level, finest first; stops when a level merges nothing.
#[must_use]
pub fn detect_hierarchy(graph: &Graph) -> Vec<Vec<u32>> {
    let n = graph.node_count();
    if n == 0 {
        return Vec::new();
    }
    let (mut adj, m2) = build_undirected_adj(graph);
    if m2 <= 0.0 {
        return vec![vec![0; n]];
    }
    let mut self_w = vec![0.0f64; n];
    let mut node_comm: Vec<u32> = (0..n as u32).collect();
    let mut levels = Vec::new();
    loop {
        let (comm, count) = louvain_partition(&adj, &self_w, m2);
        let merged = (count as usize) < adj.len();
        if !merged && !levels.is_empty() {
            break;
        }
        node_comm = node_comm.iter().map(|&c| comm[c as usize]).collect();
        levels.push(node_comm.clone());
        if !merged || count <= 1 {
            break;
        }
        (adj, self_w) = aggregate_communities(&adj, &self_w, &comm, count);
    }
    levels
}
