use crate::domain::{IngestResult, Interner, NodeId};
use crate::graph::Graph;
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};

fn find_symbols_matching(ingest: &IngestResult, interner: &Interner, name: &str) -> Vec<NodeId> {
    let ids = crate::navigate::find_symbols_by_name(name, ingest, interner);
    if !ids.is_empty() {
        return ids;
    }
    ingest
        .symbols
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            let n = interner.resolve(&s.name);
            n == name
                || interner.resolve(&s.canonical_id) == name
                || interner.resolve(&s.canonical_id).ends_with(name)
        })
        .map(|(idx, _)| idx as NodeId)
        .collect()
}

fn bfs_directed_path(graph: &Graph, from: NodeId, to: NodeId) -> Option<Vec<NodeId>> {
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let mut parent = std::collections::HashMap::new();

    queue.push_back(from);
    visited.insert(from);

    let mut found = false;
    while let Some(curr) = queue.pop_front() {
        if curr == to {
            found = true;
            break;
        }
        for (callee, _) in graph.callees(curr) {
            if visited.insert(callee) {
                parent.insert(callee, curr);
                queue.push_back(callee);
            }
        }
    }

    if !found {
        return None;
    }

    let mut path = Vec::new();
    let mut curr = to;
    while curr != from {
        path.push(curr);
        curr = *parent.get(&curr)?;
    }
    path.push(from);
    path.reverse();
    Some(path)
}

fn find_shortest_path_between_sets(
    graph: &Graph,
    from_ids: &[NodeId],
    to_ids: &[NodeId],
) -> Option<Vec<NodeId>> {
    let mut best: Option<Vec<NodeId>> = None;
    for &f in from_ids {
        for &t in to_ids {
            if f == t && (from_ids.len() > 1 || to_ids.len() > 1) {
                continue;
            }
            if let Some(p) = bfs_directed_path(graph, f, t)
                && best.as_ref().is_none_or(|b| p.len() < b.len())
            {
                best = Some(p);
            }
        }
    }
    best
}

/// Execute `path_between` tool: shortest directed call path between two symbols.
pub fn call_path_between(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    from_name: &str,
    to_name: &str,
) -> Value {
    let from_ids = find_symbols_matching(ingest, interner, from_name);
    let to_ids = find_symbols_matching(ingest, interner, to_name);

    if from_ids.is_empty() || to_ids.is_empty() {
        return json!({
            "from": from_name,
            "to": to_name,
            "from_defs": from_ids.len(),
            "to_defs": to_ids.len(),
            "reachable": false,
            "hops": 0,
            "error": "one or both symbols not found"
        });
    }
    if from_ids.len() == 1 && to_ids.len() == 1 && from_ids[0] == to_ids[0] {
        return json!({
            "from": from_name,
            "to": to_name,
            "from_defs": 1,
            "to_defs": 1,
            "reachable": true,
            "hops": 0,
            "path": [from_name]
        });
    }

    let Some(path_ids) = find_shortest_path_between_sets(graph, &from_ids, &to_ids) else {
        return json!({
            "from": from_name,
            "to": to_name,
            "from_defs": from_ids.len(),
            "to_defs": to_ids.len(),
            "reachable": false,
            "hops": 0,
            "path": []
        });
    };

    let path: Vec<&str> = path_ids
        .into_iter()
        .filter_map(|id| ingest.symbols.get(id as usize))
        .map(|sym| interner.resolve(&sym.name))
        .collect();

    json!({
        "from": from_name,
        "to": to_name,
        "from_defs": from_ids.len(),
        "to_defs": to_ids.len(),
        "reachable": true,
        "hops": path.len().saturating_sub(1),
        "path": path
    })
}

/// Execute `uses` tool: all statically resolvable use-sites of a symbol.
pub fn call_uses(ingest: &IngestResult, interner: &Interner, symbol_name: &str) -> Value {
    let mut use_sites = Vec::new();

    for r in &ingest.references {
        if interner.resolve(&r.callee_name) == symbol_name {
            let file_path = ingest
                .files
                .get(r.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            let enclosing = if r.from_symbol != crate::domain::NO_NODE {
                ingest
                    .symbols
                    .get(r.from_symbol as usize)
                    .map(|s| interner.resolve(&s.name))
            } else {
                None
            };

            use_sites.push(json!({
                "role": r.role.tag(),
                "file": file_path,
                "line": r.line,
                "enclosing": enclosing
            }));
        }
    }

    json!({
        "symbol": symbol_name,
        "count": use_sites.len(),
        "uses": use_sites
    })
}

type UndirectedPath = (Vec<NodeId>, Vec<(NodeId, NodeId)>);

fn reconstruct_undirected_path(
    mut curr: NodeId,
    parent: &std::collections::HashMap<NodeId, (NodeId, NodeId, NodeId)>,
) -> UndirectedPath {
    let mut nodes = vec![curr];
    let mut dir_edges = Vec::new();
    while let Some(&(prev, edge_from, edge_to)) = parent.get(&curr) {
        if prev == curr {
            break;
        }
        dir_edges.push((edge_from, edge_to));
        nodes.push(prev);
        curr = prev;
    }
    nodes.reverse();
    dir_edges.reverse();
    (nodes, dir_edges)
}

fn enqueue_undirected_neighbors(
    curr: NodeId,
    depth: u32,
    graph: &Graph,
    parent: &mut std::collections::HashMap<NodeId, (NodeId, NodeId, NodeId)>,
    queue: &mut std::collections::VecDeque<(NodeId, u32)>,
) {
    for (callee, _) in graph.callees(curr) {
        if let std::collections::hash_map::Entry::Vacant(e) = parent.entry(callee) {
            e.insert((curr, curr, callee));
            queue.push_back((callee, depth + 1));
        }
    }
    for (caller, _) in graph.callers(curr) {
        if let std::collections::hash_map::Entry::Vacant(e) = parent.entry(caller) {
            e.insert((curr, caller, curr));
            queue.push_back((caller, depth + 1));
        }
    }
}

fn find_undirected_path_between_sets(
    graph: &Graph,
    from_ids: &[NodeId],
    to_ids: &[NodeId],
    max_radius: u32,
) -> Option<UndirectedPath> {
    let target_set: std::collections::HashSet<NodeId> = to_ids.iter().copied().collect();
    let mut parent: std::collections::HashMap<NodeId, (NodeId, NodeId, NodeId)> =
        std::collections::HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    for &id in from_ids {
        parent.insert(id, (id, id, id));
        queue.push_back((id, 0u32));
    }
    while let Some((curr, depth)) = queue.pop_front() {
        if depth > 0 && target_set.contains(&curr) {
            return Some(reconstruct_undirected_path(curr, &parent));
        }
        if depth < max_radius {
            enqueue_undirected_neighbors(curr, depth, graph, &mut parent, &mut queue);
        }
    }
    None
}

fn build_connect_node_json(
    id: NodeId,
    defs_opt: Option<usize>,
    connects_opt: Option<usize>,
    ingest: &IngestResult,
    interner: &Interner,
) -> Option<Value> {
    let sym = ingest.symbols.get(id as usize)?;
    let name = interner.resolve(&sym.name);
    let path = ingest
        .files
        .get(sym.file_id as usize)
        .map_or("", |f| interner.resolve(&f.path));
    let mut obj = json!({
        "name": name,
        "kind": sym.kind.tag(),
        "path": path,
        "line": sym.line
    });
    if let Some(d) = defs_opt {
        obj["defs"] = json!(d);
    }
    if let Some(c) = connects_opt {
        obj["connects"] = json!(c);
    }
    Some(obj)
}

/// Execute `connect` tool: minimal connecting subgraph relating 2..16 symbols.
struct ConnectAccum {
    max_radius: u32,
    chosen_terminals: Vec<(NodeId, usize)>,
    connections: Vec<Value>,
    all_path_nodes: std::collections::BTreeSet<NodeId>,
    dir_edge_ids: std::collections::BTreeSet<(NodeId, NodeId)>,
}

fn record_connect_pair(
    i: usize,
    j: usize,
    resolved: &[(&str, Vec<NodeId>)],
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    acc: &mut ConnectAccum,
) {
    let (name_a, ref ids_a) = resolved[i];
    let (name_b, ref ids_b) = resolved[j];
    let Some((nodes, d_edges)) =
        find_undirected_path_between_sets(graph, ids_a, ids_b, acc.max_radius)
    else {
        return;
    };
    if let Some(&first_id) = nodes.first() {
        acc.chosen_terminals[i].0 = first_id;
    }
    if let Some(&last_id) = nodes.last() {
        acc.chosen_terminals[j].0 = last_id;
    }
    let path_names: Vec<&str> = nodes
        .iter()
        .filter_map(|&id| ingest.symbols.get(id as usize))
        .map(|s| interner.resolve(&s.name))
        .collect();
    acc.connections
        .push(json!({ "from": name_a, "to": name_b, "path": path_names }));
    acc.all_path_nodes.extend(nodes);
    acc.dir_edge_ids.extend(d_edges);
}

pub fn call_connect(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    symbol_names: &[String],
) -> Value {
    call_connect_with_radius(ingest, graph, interner, symbol_names, 6)
}

pub fn call_connect_with_radius(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    symbol_names: &[String],
    max_radius: u32,
) -> Value {
    let resolved: Vec<(&str, Vec<NodeId>)> = symbol_names
        .iter()
        .filter_map(|name| {
            let ids = find_symbols_matching(ingest, interner, name);
            (!ids.is_empty()).then_some((name.as_str(), ids))
        })
        .collect();
    let mut acc = ConnectAccum {
        max_radius,
        chosen_terminals: resolved
            .iter()
            .map(|(_, ids)| (ids[0], ids.len()))
            .collect(),
        connections: Vec::new(),
        all_path_nodes: std::collections::BTreeSet::new(),
        dir_edge_ids: std::collections::BTreeSet::new(),
    };

    for i in 0..resolved.len() {
        for j in (i + 1)..resolved.len() {
            record_connect_pair(i, j, &resolved, ingest, graph, interner, &mut acc);
        }
    }

    let term_id_set: std::collections::HashSet<NodeId> =
        acc.chosen_terminals.iter().map(|&(id, _)| id).collect();
    let terminals_json: Vec<Value> = acc
        .chosen_terminals
        .iter()
        .filter_map(|&(id, defs)| build_connect_node_json(id, Some(defs), None, ingest, interner))
        .collect();
    let steiner_json: Vec<Value> = acc
        .all_path_nodes
        .into_iter()
        .filter(|id| !term_id_set.contains(id))
        .filter_map(|id| {
            let deg = graph.in_degree(id) + graph.out_degree(id);
            build_connect_node_json(id, None, Some(deg), ingest, interner)
        })
        .collect();
    let edges_json: Vec<Value> = acc
        .dir_edge_ids
        .into_iter()
        .filter_map(|(u, v)| {
            let su = ingest.symbols.get(u as usize)?;
            let sv = ingest.symbols.get(v as usize)?;
            Some(json!({
                "from": interner.resolve(&su.name),
                "to": interner.resolve(&sv.name)
            }))
        })
        .collect();

    json!({
        "symbols": symbol_names,
        "resolved": resolved.len(),
        "terminals": terminals_json,
        "steiner": steiner_json,
        "edges": edges_json,
        "connections": acc.connections
    })
}

/// Execute `analyze` and `rank_by` tools.
pub fn call_analyze(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    ranks: &[f64],
    top_k: Option<usize>,
    rank_by: Option<&str>,
) -> Value {
    let mode = rank_by.unwrap_or("pagerank");
    let mut indices: Vec<usize> = (0..ingest.symbols.len()).collect();

    match mode {
        "degree" => {
            indices.sort_by_key(|&idx| {
                let id = idx as NodeId;
                std::cmp::Reverse(graph.in_degree(id) + graph.out_degree(id))
            });
        }
        "ccx" => {
            indices.sort_by_key(|&idx| {
                std::cmp::Reverse(ingest.symbols.get(idx).map(|s| s.ccx).unwrap_or(0))
            });
        }
        _ => {
            indices.sort_by(|&a, &b| {
                let ra = ranks.get(a).copied().unwrap_or(0.0);
                let rb = ranks.get(b).copied().unwrap_or(0.0);
                rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    let limit = top_k.unwrap_or(20).min(indices.len());
    let mut symbols_out = Vec::new();

    for &idx in &indices[..limit] {
        if let Some(sym) = ingest.symbols.get(idx) {
            let id = idx as NodeId;
            let name = interner.resolve(&sym.name);
            let path = ingest
                .files
                .get(sym.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            let callees: Vec<&str> = graph
                .callees(id)
                .filter_map(|(c_id, _)| ingest.symbols.get(c_id as usize))
                .map(|cs| interner.resolve(&cs.name))
                .collect();

            symbols_out.push(json!({
                "id": id,
                "name": name,
                "kind": sym.kind.tag(),
                "path": path,
                "line": sym.line,
                "in_degree": graph.in_degree(id),
                "out_degree": graph.out_degree(id),
                "ccx": sym.ccx,
                "rank": (ranks.get(idx).copied().unwrap_or(0.0) * 10000.0).round() / 10000.0,
                "callees": callees
            }));
        }
    }

    json!({
        "ranked_by": mode,
        "total_symbols": ingest.symbols.len(),
        "symbols": symbols_out
    })
}

/// Execute `situational_awareness` tool.
fn collect_ranked_cochange_partners(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    changed_files: &[String],
    commits: Option<&[crate::git::history::CommitInfo]>,
) -> Vec<String> {
    let coupling = crate::git::cochange::StaticCouplingMap::build(ingest, interner, Some(graph));
    let opts = crate::git::cochange::CochangeOptions::default();
    let commits_empty = Vec::new();
    let commit_slice = commits.unwrap_or(&commits_empty);

    let mut partner_deg: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for file_path in changed_files {
        let partners = crate::git::cochange::mine_cochange_for_file(
            file_path,
            commit_slice,
            Some(&coupling),
            Some(ingest),
            Some(interner),
            &opts,
        );
        for p in partners {
            if !changed_files.iter().any(|c| c == &p.file_path) {
                let entry = partner_deg.entry(p.file_path).or_insert(p.deg);
                *entry = entry.max(p.deg);
            }
        }
    }
    let mut ranked: Vec<(String, f64)> = partner_deg.into_iter().collect();
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    ranked
        .into_iter()
        .take(20)
        .map(|(path, deg)| format!("{path} ({:.0}%)", (deg * 100.0).round()))
        .collect()
}

fn format_affected_test_label(t: &crate::gate::affected::AffectedTest) -> String {
    if t.is_direct_change {
        format!("[changed] {}", t.path)
    } else {
        format!("[hops={}] {}", t.hops, t.path)
    }
}

/// Execute `situational_awareness` tool.
pub fn call_situational_awareness(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    changed_files: &[String],
    commits: Option<&[crate::git::history::CommitInfo]>,
) -> Value {
    let specs: Vec<&str> = changed_files.iter().map(|s| s.as_str()).collect();
    let (affected_tests, blast_radius) = if specs.is_empty() {
        (Vec::new(), 0)
    } else {
        let report = crate::gate::affected::find_affected_tests(&specs, ingest, graph, interner);
        let tests = report
            .tests
            .iter()
            .map(format_affected_test_label)
            .collect();
        (tests, report.reached_symbols)
    };
    let cochange_partners =
        collect_ranked_cochange_partners(ingest, graph, interner, changed_files, commits);

    json!({
        "files_touched": changed_files,
        "tests_to_run": affected_tests,
        "blast_radius": blast_radius,
        "forgotten_partners": cochange_partners
    })
}

/// Execute `from_trace` tool.
pub fn call_from_trace(ingest: &IngestResult, interner: &Interner, trace_text: &str) -> Value {
    let mut suspects = Vec::new();

    for line in trace_text.lines() {
        let trimmed = line.trim();
        for sym in &ingest.symbols {
            let name = interner.resolve(&sym.name);
            if name.len() >= 3 && trimmed.contains(name) {
                let file_path = ingest
                    .files
                    .get(sym.file_id as usize)
                    .map(|f| interner.resolve(&f.path))
                    .unwrap_or("");
                suspects.push(json!({
                    "name": name,
                    "kind": sym.kind.tag(),
                    "file": file_path,
                    "line": sym.line,
                    "trace_line": trimmed
                }));
                break;
            }
        }
    }

    json!({
        "matched_frames": suspects.len(),
        "suspects": suspects
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_between_not_found() {
        let ingest = IngestResult::new();
        let interner = Interner::new();
        let graph = Graph::build(&ingest, &interner);
        let res = call_path_between(&ingest, &graph, &interner, "a", "b");
        assert_eq!(res["reachable"], false);
    }

    #[test]
    fn test_path_between_connected() {
        let interner = Interner::new();
        let registry = crate::ingest::LanguageRegistry::new().unwrap();
        let code = b"fn alpha() { beta(); } fn beta() {}";
        let ingest = crate::ingest::ingest_in_memory_files(
            &[("src/lib.rs", crate::domain::Lang::Rust, code)],
            &registry,
            &interner,
        );
        let graph = Graph::build(&ingest, &interner);
        let res = call_path_between(&ingest, &graph, &interner, "alpha", "beta");
        assert_eq!(res["reachable"], true);
        assert_eq!(res["hops"], 1);
    }
}
