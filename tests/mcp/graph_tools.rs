use ripwire_rs::query::graph::call_path_between;
use ripwire_rs::{Graph, IngestResult, Interner, Lang, LanguageRegistry, ingest_in_memory_files};

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
    let registry = LanguageRegistry::new().unwrap();
    let code = b"fn alpha() { beta(); } fn beta() {}";
    let ingest = ingest_in_memory_files(&[("src/lib.rs", Lang::Rust, code)], &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let res = call_path_between(&ingest, &graph, &interner, "alpha", "beta");
    assert_eq!(res["reachable"], true);
    assert_eq!(res["hops"], 1);
}
