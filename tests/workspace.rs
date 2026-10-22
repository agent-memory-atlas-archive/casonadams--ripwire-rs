use ripwire_rs::crawler::CrawlerOptions;
use ripwire_rs::domain::Interner;
use ripwire_rs::workspace::{Workspace, analyze_ingest, is_analysis_cacheable};
use std::path::PathBuf;

#[test]
fn test_is_analysis_cacheable_rules() {
    let default_opts = CrawlerOptions::default();
    assert!(is_analysis_cacheable(&default_opts, None));

    let dummy_path = PathBuf::from("index.scip");
    assert!(!is_analysis_cacheable(&default_opts, Some(&dummy_path)));

    let custom_opts = CrawlerOptions {
        ignore_tests: true,
        ..Default::default()
    };
    assert!(!is_analysis_cacheable(&custom_opts, None));
    assert!(!is_analysis_cacheable(&custom_opts, Some(&dummy_path)));
}

#[test]
fn test_analyze_ingest_empty_and_in_memory() {
    let empty_ingest = ripwire_rs::domain::IngestResult::new();
    let interner = Interner::new();
    let (empty_ingest, empty_graph, empty_pr, _) =
        analyze_ingest(empty_ingest, interner, None).unwrap();
    assert_eq!(empty_ingest.files.len(), 0);
    assert_eq!(empty_graph.node_count(), 0);
    assert_eq!(empty_pr.ranks.len(), 0);

    let registry = ripwire_rs::LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let files: &[(&str, ripwire_rs::Lang, &[u8])] = &[(
        "sample.rs",
        ripwire_rs::Lang::Rust,
        b"pub fn calculate(x: i32, y: i32) -> i32 { x * y }\n",
    )];
    let ingest = ripwire_rs::ingest_in_memory_files(files, &registry, &interner);
    let (ingest, graph, pr, interner) = analyze_ingest(ingest, interner, None).unwrap();
    assert_eq!(ingest.files.len(), 1);
    assert!(!ingest.symbols.is_empty(), "symbol should be extracted");
    assert_eq!(graph.node_count(), ingest.symbols.len());
    assert_eq!(pr.ranks.len(), graph.node_count());
    let sym_name = interner.resolve(&ingest.symbols[0].name);
    assert_eq!(sym_name, "calculate");
}

#[test]
fn test_analyze_ingest_invalid_scip_errors() {
    let invalid_bytes = b"not a valid scip protobuf payload";
    let ingest = ripwire_rs::domain::IngestResult::new();
    let interner = Interner::new();
    let err = analyze_ingest(ingest, interner, Some(invalid_bytes));
    assert!(err.is_err());
}

#[test]
fn test_analyze_ingest_multi_file_graph_and_pagerank() {
    let registry = ripwire_rs::LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let files: &[(&str, ripwire_rs::Lang, &[u8])] = &[
        (
            "caller.rs",
            ripwire_rs::Lang::Rust,
            b"pub fn run() { helper(); }\nfn helper() {}\n",
        ),
        ("callee.rs", ripwire_rs::Lang::Rust, b"pub fn worker() {}\n"),
    ];
    let ingest = ripwire_rs::ingest_in_memory_files(files, &registry, &interner);
    let (ingest, graph, pr, interner) = analyze_ingest(ingest, interner, None).unwrap();
    assert_eq!(ingest.files.len(), 2);
    assert!(graph.node_count() >= 3);
    assert_eq!(pr.ranks.len(), graph.node_count());
    for sym in &ingest.symbols {
        let name = interner.resolve(&sym.name);
        assert!(["run", "helper", "worker"].contains(&name));
    }
}

#[test]
fn test_workspace_builder_and_query_context() {
    let interner = Interner::new();
    let ws = Workspace::from_ingest(
        "dummy_root",
        ripwire_rs::domain::IngestResult::new(),
        interner,
    );
    assert_eq!(ws.root, "dummy_root");
    assert_eq!(ws.ingest.files.len(), 0);

    let ws = ws.with_commits(Vec::new());
    assert!(ws.commits.is_some());

    let sources: [(&str, &[u8]); 0] = [];
    let qc = ws.query_context(&sources);
    assert_eq!(qc.root, "dummy_root");
    assert_eq!(qc.ingest.files.len(), 0);
}
