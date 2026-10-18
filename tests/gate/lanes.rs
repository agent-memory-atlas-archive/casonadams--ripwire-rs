use ripwire_rs::gate::lanes::compute_plan_lanes;
use ripwire_rs::{Graph, IngestResult, Interner, PageRankResult};

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
