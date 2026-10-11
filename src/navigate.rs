use crate::domain::{IngestResult, Interner, NodeId};
use crate::graph::Graph;
use serde::Serialize;
use std::collections::{HashSet, VecDeque};

/// Detailed navigation information for a symbol.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SymbolNavInfo {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub line: u32,
    pub kind: &'static str,
    pub rank: f64,
    pub weight: f32,
}

/// 1-hop neighborhood of a symbol containing both incoming callers and outgoing callees.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NavNeighborhood {
    pub target: SymbolNavInfo,
    pub callers: Vec<SymbolNavInfo>,
    pub callees: Vec<SymbolNavInfo>,
}

/// Transitive reachability impact node with hop distance.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImpactNode {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub line: u32,
    pub kind: &'static str,
    pub rank: f64,
    pub hops: u32,
}

/// Find symbol IDs by exact name match.
#[must_use]
pub fn find_symbols_by_name(name: &str, ingest: &IngestResult, interner: &Interner) -> Vec<NodeId> {
    ingest
        .symbols
        .iter()
        .filter(|s| interner.resolve(&s.name) == name)
        .map(|s| s.id)
        .collect()
}

/// Build `SymbolNavInfo` for a node.
#[must_use]
pub fn get_symbol_nav_info(
    id: NodeId,
    ingest: &IngestResult,
    ranks: &[f64],
    interner: &Interner,
    weight: f32,
) -> Option<SymbolNavInfo> {
    let sym = ingest.symbols.get(id as usize)?;
    let name = interner.resolve(&sym.name).to_string();
    let file = ingest.files.get(sym.file_id as usize)?;
    let path = interner.resolve(&file.path).to_string();
    let rank = ranks.get(id as usize).copied().unwrap_or(0.0);

    Some(SymbolNavInfo {
        id,
        name,
        path,
        line: sym.line,
        kind: sym.kind.tag(),
        rank,
        weight,
    })
}

/// Compute 1-hop neighborhood (direct callers and callees) for a target symbol.
#[must_use]
pub fn navigate_1hop(
    sym_id: NodeId,
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
) -> Option<NavNeighborhood> {
    let target = get_symbol_nav_info(sym_id, ingest, ranks, interner, 1.0)?;

    let mut callers = Vec::new();
    for (caller_id, weight) in graph.callers(sym_id) {
        if let Some(info) = get_symbol_nav_info(caller_id, ingest, ranks, interner, weight) {
            callers.push(info);
        }
    }
    callers.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.id.cmp(&b.id)));

    let mut callees = Vec::new();
    for (callee_id, weight) in graph.callees(sym_id) {
        if let Some(info) = get_symbol_nav_info(callee_id, ingest, ranks, interner, weight) {
            callees.push(info);
        }
    }
    callees.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.id.cmp(&b.id)));

    Some(NavNeighborhood {
        target,
        callers,
        callees,
    })
}

fn create_impact_node(
    curr: NodeId,
    depth: u32,
    ingest: &IngestResult,
    ranks: &[f64],
    interner: &Interner,
) -> Option<ImpactNode> {
    let sym = ingest.symbols.get(curr as usize)?;
    let name = interner.resolve(&sym.name).to_string();
    let path = ingest
        .files
        .get(sym.file_id as usize)
        .map(|f| interner.resolve(&f.path).to_string())
        .unwrap_or_default();
    let rank = ranks.get(curr as usize).copied().unwrap_or(0.0);

    Some(ImpactNode {
        id: curr,
        name,
        path,
        line: sym.line,
        kind: sym.kind.tag(),
        rank,
        hops: depth,
    })
}

/// Compute transitive blast radius / caller closure backward along in-edges using BFS.
#[must_use]
pub fn transitive_impact(
    seed_id: NodeId,
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    max_depth: Option<u32>,
) -> Vec<ImpactNode> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut results = Vec::new();

    visited.insert(seed_id);
    queue.push_back((seed_id, 0u32));

    while let Some((curr, depth)) = queue.pop_front() {
        if curr != seed_id {
            if let Some(node) = create_impact_node(curr, depth, ingest, ranks, interner) {
                results.push(node);
            }
        }

        if max_depth.is_some_and(|limit| depth >= limit) {
            continue;
        }

        for (caller_id, _) in graph.callers(curr) {
            if visited.insert(caller_id) {
                queue.push_back((caller_id, depth + 1));
            }
        }
    }

    results.sort_by(|a, b| a.hops.cmp(&b.hops).then_with(|| b.rank.total_cmp(&a.rank)));
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        FileInfo, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol,
    };

    #[test]
    fn test_navigate_1hop_and_impact() {
        let interner = Interner::new();
        let f0 = FileInfo {
            id: 0,
            path: interner.intern("src/lib.rs"),
            lang: Lang::Rust,
            line_count: 50,
            byte_size: 500,
        };

        let sym_a = Symbol::new(
            0,
            interner.intern("a"),
            interner.intern("c_a"),
            SymKind::Function,
            Lang::Rust,
            0,
            1,
        );
        let sym_b = Symbol::new(
            1,
            interner.intern("b"),
            interner.intern("c_b"),
            SymKind::Function,
            Lang::Rust,
            0,
            5,
        );
        let sym_c = Symbol::new(
            2,
            interner.intern("c"),
            interner.intern("c_c"),
            SymKind::Function,
            Lang::Rust,
            0,
            10,
        );

        // a calls b; b calls c
        let ref_ab = Reference::new(0, interner.intern("b"), 0, 2, Lang::Rust, RefRole::Call);
        let ref_bc = Reference::new(1, interner.intern("c"), 0, 6, Lang::Rust, RefRole::Call);

        let mut ingest = IngestResult::new();
        ingest.files.push(f0);
        ingest.symbols.extend_from_slice(&[sym_a, sym_b, sym_c]);
        ingest.references.extend_from_slice(&[ref_ab, ref_bc]);

        let graph = Graph::build(&ingest, &interner);
        let ranks = vec![0.2, 0.3, 0.5];

        let matched = find_symbols_by_name("b", &ingest, &interner);
        assert_eq!(matched, vec![1]);

        let nav = navigate_1hop(1, &ingest, &graph, &ranks, &interner).unwrap();
        assert_eq!(nav.target.name, "b");
        assert_eq!(nav.callers.len(), 1);
        assert_eq!(nav.callers[0].name, "a");
        assert_eq!(nav.callees.len(), 1);
        assert_eq!(nav.callees[0].name, "c");

        // Impact of c: callers reach b (1 hop) and a (2 hops)
        let impact = transitive_impact(2, &ingest, &graph, &ranks, &interner, None);
        assert_eq!(impact.len(), 2);
        assert_eq!(impact[0].name, "b");
        assert_eq!(impact[0].hops, 1);
        assert_eq!(impact[1].name, "a");
        assert_eq!(impact[1].hops, 2);
    }
}
