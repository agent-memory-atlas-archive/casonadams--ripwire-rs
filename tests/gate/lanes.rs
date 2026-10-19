use ripwire_rs::gate::lanes::{
    compute_plan_lanes, compute_plan_lanes_with_brief, measure_partition_overlap,
    plan_task_partition,
};
use ripwire_rs::{
    Graph, IngestResult, Interner, Lang, LanguageRegistry, PageRankConfig, ingest_in_memory_files,
};
use std::collections::BTreeSet;

#[test]
fn test_plan_lanes_partition_and_brief() {
    let empty_ingest = IngestResult::new();
    let empty_interner = Interner::new();
    let empty_graph = Graph::build(&empty_ingest, &empty_interner);
    let empty_pr = empty_graph.page_rank(&PageRankConfig::default());
    let empty_val = compute_plan_lanes(
        ".",
        2,
        "refactor parser",
        &empty_ingest,
        &empty_graph,
        &empty_pr,
        &empty_interner,
    );
    assert_eq!(empty_val["verb"], "plan-lanes");
    assert_eq!(empty_val["requested"], 2);

    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let files = [
        (
            "src/auth.rs",
            Lang::Rust,
            b"pub fn login_user() { verify_token(); }\npub fn verify_token() {}\npub struct AuthConfig { pub ttl: u32 }\npub struct TokenPair { pub id: u32 }\n" as &[u8],
        ),
        (
            "src/billing.rs",
            Lang::Rust,
            b"pub fn charge_card() { record_ledger(); }\npub fn record_ledger() {}\npub fn refund_card() { charge_card(); }\n" as &[u8],
        ),
        (
            "tests/auth_test.rs",
            Lang::Rust,
            b"fn test_login() { login_user(); charge_card(); }\n" as &[u8],
        ),
    ];
    let ingest = ingest_in_memory_files(&files, &reg, &interner);
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&PageRankConfig::default());

    let plan = plan_task_partition(
        "login verify charge",
        4,
        &ingest,
        &graph,
        &pr.ranks,
        &interner,
    );
    assert!(!plan.core_ids.is_empty());
    assert!(!plan.groups.is_empty());

    let val = compute_plan_lanes(
        ".",
        2,
        "login verify charge",
        &ingest,
        &graph,
        &pr,
        &interner,
    );
    assert_eq!(val["source"], "partition");
    assert_eq!(val["carve"]["strategy"], "louvain-community");
    assert!(val["lanes"].as_array().is_some_and(|a| !a.is_empty()));
    assert!(
        val["landing_order"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
    );

    let brief_lines = vec![
        "login_user verify_token".to_string(),
        "verify_token charge_card".to_string(),
    ];
    let brief_val =
        compute_plan_lanes_with_brief(".", &brief_lines, &ingest, &graph, &pr, &interner);
    assert_eq!(brief_val["source"], "brief");
    assert_eq!(brief_val["lane_count"], 2);
    assert_eq!(brief_val["pairs"].as_array().map(Vec::len), Some(1));

    let s1: BTreeSet<u32> = [0, 1, 2].into_iter().collect();
    let s2: BTreeSet<u32> = [2, 3].into_iter().collect();
    let core: BTreeSet<u32> = [0, 4].into_iter().collect();
    let ov = measure_partition_overlap(&[s1, s2], &core);
    assert_eq!(ov.shared_count, 1);
    assert_eq!(ov.union_count, 4);
    assert!((ov.core_leak - 0.5).abs() < 1e-6);
}
