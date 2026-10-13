use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::rank::PageRankResult;
use serde_json::{Value, json};

pub fn compute_plan_lanes(
    root: &str,
    requested: usize,
    task: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
) -> Value {
    let at_stamp = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
    let n = requested.max(1);
    let ranked = crate::mcp::retrieval::call_for_task(
        ingest,
        interner,
        &pr.ranks,
        task,
        Some(n * 10),
        None,
        false,
    );
    let syms = ranked["symbols"].as_array().cloned().unwrap_or_default();

    let mut lanes = Vec::new();
    for i in 0..n {
        let lane_syms: Vec<Value> = syms
            .iter()
            .enumerate()
            .filter(|(idx, _)| idx % n == i)
            .map(|(_, s)| s.clone())
            .collect();
        lanes.push(json!({
            "lane": i + 1,
            "target": format!("worktree-lane-{}", i + 1),
            "symbols_count": lane_syms.len(),
            "symbols": lane_syms
        }));
    }

    json!({
        "v": 1,
        "verb": "plan-lanes",
        "at": at_stamp,
        "root": root,
        "task": task,
        "source": "partition",
        "requested": n,
        "lane_count": n,
        "claim_key": "path+scope+name",
        "on_conflict": "producing-lane-rebases",
        "corpus": {
            "files": ingest.files.len(),
            "symbols": ingest.symbols.len(),
            "edges": graph.edge_count()
        },
        "lanes": lanes
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_lanes_partition() {
        let ingest = IngestResult::new();
        let interner = Interner::new();
        let graph = Graph::build(&ingest, &interner);
        let pr = PageRankResult {
            ranks: Vec::new(),
            iterations: 0,
            converged: true,
        };
        let val = compute_plan_lanes(".", 2, "refactor parser", &ingest, &graph, &pr, &interner);
        assert_eq!(val["verb"], "plan-lanes");
        assert_eq!(val["requested"], 2);
    }
}
