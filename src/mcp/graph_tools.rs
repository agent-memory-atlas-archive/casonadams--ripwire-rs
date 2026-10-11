use crate::domain::{IngestResult, Interner, NodeId};
use crate::graph::Graph;
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};

fn find_symbol_by_name(ingest: &IngestResult, interner: &Interner, name: &str) -> Option<NodeId> {
    ingest
        .symbols
        .iter()
        .position(|s| {
            let n = interner.resolve(&s.name);
            n == name
                || interner.resolve(&s.canonical_id) == name
                || interner.resolve(&s.canonical_id).ends_with(name)
        })
        .map(|idx| idx as NodeId)
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

/// Execute `path_between` tool: shortest directed call path between two symbols.
pub fn call_path_between(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    from_name: &str,
    to_name: &str,
) -> Value {
    let from_id = find_symbol_by_name(ingest, interner, from_name);
    let to_id = find_symbol_by_name(ingest, interner, to_name);

    let (Some(from), Some(to)) = (from_id, to_id) else {
        return json!({
            "from": from_name,
            "to": to_name,
            "reachable": false,
            "hops": 0,
            "error": "one or both symbols not found"
        });
    };
    if from == to {
        return json!({
            "from": from_name,
            "to": to_name,
            "reachable": true,
            "hops": 0,
            "path": [from_name]
        });
    }

    let Some(path_ids) = bfs_directed_path(graph, from, to) else {
        return json!({
            "from": from_name,
            "to": to_name,
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

/// Execute `connect` tool: minimal connecting subgraph relating 2..16 symbols.
pub fn call_connect(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    symbol_names: &[String],
) -> Value {
    let mut resolved_ids = Vec::new();
    for name in symbol_names {
        if let Some(id) = find_symbol_by_name(ingest, interner, name) {
            resolved_ids.push((name.as_str(), id));
        }
    }

    let mut edges = Vec::new();
    for (i, &(name_a, _)) in resolved_ids.iter().enumerate() {
        for &(name_b, _) in &resolved_ids[i + 1..] {
            let path_val = call_path_between(ingest, graph, interner, name_a, name_b);
            if path_val["reachable"].as_bool() == Some(true) {
                edges.push(json!({ "from": name_a, "to": name_b, "path": path_val["path"] }));
            }
        }
    }

    json!({
        "symbols": symbol_names,
        "resolved": resolved_ids.len(),
        "connections": edges
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
pub fn call_situational_awareness(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    changed_files: &[String],
    commits: Option<&[crate::git::history::CommitInfo]>,
) -> Value {
    let mut affected_tests = Vec::new();
    let mut blast_radius = 0;

    let specs: Vec<&str> = changed_files.iter().map(|s| s.as_str()).collect();
    if !specs.is_empty() {
        let report = crate::gate::affected::find_affected_tests(&specs, ingest, graph, interner);
        for t in report.tests {
            affected_tests.push(t.path);
        }
        blast_radius = report.reached_symbols;
    }
    affected_tests.sort();
    affected_tests.dedup();

    let coupling = crate::git::cochange::StaticCouplingMap::build(ingest, interner, Some(graph));
    let opts = crate::git::cochange::CochangeOptions::default();
    let commits_empty = Vec::new();
    let commit_slice = commits.unwrap_or(&commits_empty);

    let mut cochange_partners = Vec::new();
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
            cochange_partners.push(p.file_path);
        }
    }
    cochange_partners.sort();
    cochange_partners.dedup();

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
