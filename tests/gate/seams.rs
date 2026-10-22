use ripwire_rs::gate::seams::find_integration_seams;
use ripwire_rs::serialize::{PageRequest, XmlReport};
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, ingest_in_memory_files};

#[test]
fn test_find_integration_seams_across_directories() {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[
            (
                "src/gate/seams.rs",
                Lang::Rust,
                b"pub fn caller_fn() { callee_fn(); }\n",
            ),
            ("lib.rs", Lang::Rust, b"pub fn callee_fn() {}\n"),
        ],
        &reg,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    let rep = find_integration_seams(&ingest, &graph, &interner, PageRequest::default());
    assert_eq!(rep.groups.len(), 1);
    assert_eq!(rep.groups[0].from_dir, "src/gate");
    assert_eq!(rep.groups[0].to_dir, ".");
}

#[test]
fn test_find_integration_seams_honors_limit_and_offset() {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[
            (
                "src/gate/seams.rs",
                Lang::Rust,
                b"pub fn caller_fn() { callee_fn(); }\n",
            ),
            (
                "src/query/safety.rs",
                Lang::Rust,
                b"pub fn caller_query() { callee_fn(); }\n",
            ),
            ("lib.rs", Lang::Rust, b"pub fn callee_fn() {}\n"),
        ],
        &reg,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    let page = PageRequest {
        limit: Some(1),
        offset: Some(1),
    };
    let rep = find_integration_seams(&ingest, &graph, &interner, page);
    assert_eq!(rep.groups.len(), 2);
    let window = rep.page_window().expect("seams report is paged");
    assert_eq!((window.shown, window.total, window.begin), (1, 2, 1));
    assert!(!window.has_more);
}
