use ripwire_rs::gate::pr_context::{
    FileHistory, PrContextReport, PrContextRequest, PrFileContext, compute_pr_context_from_facts,
    serialize_pr_context_xml,
};
use ripwire_rs::serialize::GraphGauges;
use ripwire_rs::{FileInfo, Graph, IngestResult, Interner, Lang, SymKind, Symbol};

#[test]
fn test_serialize_pr_context_xml() {
    let report = PrContextReport {
        base_ref: Some("main".to_string()),
        at_stamp: "abc123def".to_string(),
        files_count: 1,
        files: vec![PrFileContext {
            path: "src/lib.rs".to_string(),
            symbol_count: 5,
            changed_symbols: vec!["run_cli".to_string(), "init".to_string()],
            dependents_count: 3,
            files_other_count: 0,
            tests_count: 2,
            history: Some(FileHistory {
                commits: 4,
                partners: 0,
                authors: 1,
                bus_factor: 1,
            }),
        }],
        gauges: GraphGauges::default(),
        page: ripwire_rs::serialize::PageRequest::default(),
    };
    let mut out = Vec::new();
    serialize_pr_context_xml(&report, &mut out).expect("serialize should succeed");
    let xml = String::from_utf8(out).expect("valid utf-8");
    assert!(xml.contains("schema=\"ripwire-rs.pr-context/v1\""));
    assert!(xml.contains("base=\"main\" direction=\"worktree-since-merge-base\""));
    assert!(xml.contains(" at=\"abc123def\""));
    assert!(xml.contains("<file p=\"src/lib.rs\" symbols=\"5\">"));
    assert!(xml.contains("<s n=\"run_cli\"/>"));
    assert!(xml.contains("<s n=\"init\"/>"));
    assert!(xml.contains("<impact dependents=\"3\" files_other=\"0\"/>"));
    assert!(xml.contains("<tests count=\"2\"/>"));
    assert!(xml.contains("<cochange commits=\"4\" partners=\"0\"/>"));
    assert!(xml.contains("<owners authors=\"1\" bf=\"1\"/>"));
}

#[test]
fn test_compute_pr_context_report() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    let p = interner.intern("src/worker.rs");
    ingest.files.push(FileInfo {
        id: 0,
        path: p,
        lang: Lang::Rust,
        byte_size: 100,
        line_count: 10,
    });
    let sname = interner.intern("run_job");
    ingest.symbols.push(Symbol::new(
        0,
        sname,
        sname,
        SymKind::Function,
        Lang::Rust,
        0,
        5,
    ));
    let graph = Graph::build(&ingest, &interner);
    let files = ["src/worker.rs".to_string()];
    let req = PrContextRequest {
        changed_files: &files,
        base_ref: Some("HEAD~1"),
        at_stamp: "",
        commits: &[],
        page: Default::default(),
    };
    let report = compute_pr_context_from_facts(&req, &ingest, &graph, &interner);
    assert_eq!(report.base_ref.as_deref(), Some("HEAD~1"));
    assert_eq!(report.files[0].history, None);
    assert_eq!(report.files_count, 1);
    assert_eq!(report.files[0].path, "src/worker.rs");
    assert_eq!(report.files[0].changed_symbols, vec!["run_job"]);
}

#[test]
fn test_pr_context_file_without_history_claims_nothing() {
    let report = PrContextReport {
        base_ref: None,
        at_stamp: String::new(),
        files_count: 1,
        files: vec![PrFileContext {
            path: "src/fresh.rs".to_string(),
            symbol_count: 1,
            changed_symbols: Vec::new(),
            dependents_count: 0,
            files_other_count: 0,
            tests_count: 0,
            history: None,
        }],
        gauges: GraphGauges::default(),
        page: ripwire_rs::serialize::PageRequest::default(),
    };
    let mut out = Vec::new();
    serialize_pr_context_xml(&report, &mut out).expect("serialize should succeed");
    let xml = String::from_utf8(out).expect("valid utf-8");
    assert!(xml.contains("base=\"working-tree\" direction=\"worktree-since-head\""));
    assert!(!xml.contains(" at=\""));
    assert!(!xml.contains("<cochange"));
    assert!(!xml.contains("<owners"));
}
