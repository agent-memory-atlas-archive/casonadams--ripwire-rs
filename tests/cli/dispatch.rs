use ripwire_rs::commands::batch::execute_batch_content;
use ripwire_rs::query::graph::execute_path_query;
use ripwire_rs::{
    Graph, IngestResult, Interner, Lang, LanguageRegistry, PageRankConfig, ingest_in_memory_files,
};

fn setup_in_memory_fixture() -> (IngestResult, Graph, Interner) {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[(
            "lib.rs",
            Lang::Rust,
            b"pub fn run_cli() { helper(); } fn helper() {}",
        )],
        &reg,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    (ingest, graph, interner)
}

#[test]
fn test_path_queries_report_fixture_edges_and_reject_missing_endpoints() {
    let (ingest, graph, interner) = setup_in_memory_fixture();
    let mut out = Vec::new();
    execute_path_query("run_cli,helper", &ingest, &graph, &interner, &mut out).unwrap();
    let xml = String::from_utf8(out).unwrap();
    assert!(xml.contains("reachable=\"1\""), "{xml}");
    assert!(xml.contains("hops=\"1\""), "{xml}");

    let mut out = Vec::new();
    execute_path_query("run_cli,missing", &ingest, &graph, &interner, &mut out).unwrap_err();
    assert!(out.is_empty());
}

#[test]
fn test_safe_delete_reports_callers_and_missing_symbols() {
    let (ingest, graph, interner) = setup_in_memory_fixture();
    let mut out = Vec::new();
    let facts = ripwire_rs::query::safety::SafetyContext {
        root: ":memory:",
        ingest: &ingest,
        graph: &graph,
        interner: &interner,
    };
    ripwire_rs::commands::common::safety_queries::execute_safe_delete_query(
        &facts,
        "helper",
        ripwire_rs::serialize::PageRequest::default(),
        &mut out,
    )
    .unwrap();
    let xml = String::from_utf8(out).unwrap();
    assert!(
        xml.contains("<safe-delete schema=\"ripwire-rs.safe-delete/v1\""),
        "{xml}"
    );
    assert!(xml.contains("run_cli"), "{xml}");

    let mut out = Vec::new();
    let facts = ripwire_rs::query::safety::SafetyContext {
        root: ":memory:",
        ingest: &ingest,
        graph: &graph,
        interner: &interner,
    };
    ripwire_rs::commands::common::safety_queries::execute_safe_delete_query(
        &facts,
        "missing",
        ripwire_rs::serialize::PageRequest::default(),
        &mut out,
    )
    .unwrap_err();
    assert!(out.is_empty());
}

#[test]
fn test_batch_content_runs_navigation_queries_in_memory() {
    let (ingest, graph, interner) = setup_in_memory_fixture();
    let pr = graph.page_rank(&PageRankConfig::default());
    let mut out = Vec::new();
    execute_batch_content(
        ".",
        "# fixture queries\n\ncallers:helper\ncallees:run_cli\n",
        &ingest,
        &graph,
        &pr,
        &interner,
        &mut out,
    )
    .unwrap();
    let xml = String::from_utf8(out).unwrap();
    assert!(xml.contains("count=\"2\""), "{xml}");
    assert!(
        xml.contains("<callers schema=\"ripwire-rs.callers/v1\" of=\"helper\""),
        "{xml}"
    );
    assert!(
        xml.contains("<callees schema=\"ripwire-rs.callees/v1\" of=\"run_cli\""),
        "{xml}"
    );
    assert!(xml.ends_with("</batch>\n"), "{xml}");
}

#[test]
fn test_exercises_reports_production_callees() {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[
            ("lib.rs", Lang::Rust, b"pub fn run_cli() {}"),
            (
                "tests/check.rs",
                Lang::Rust,
                b"fn test_run() { run_cli(); }",
            ),
        ],
        &reg,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    let mut out = Vec::new();
    ripwire_rs::commands::common::safety_queries::execute_exercises_query(
        ":memory:",
        "tests/check.rs",
        &ingest,
        &graph,
        &interner,
        &mut out,
    )
    .unwrap();
    let xml = String::from_utf8(out).unwrap();
    assert!(
        xml.contains("<exercises schema=\"ripwire-rs.exercises/v1\""),
        "{xml}"
    );
    assert!(xml.contains("run_cli"), "{xml}");
    assert!(xml.contains("lib.rs"), "{xml}");
}
