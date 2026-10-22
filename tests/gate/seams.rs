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
        budget: None,
    };
    let rep = find_integration_seams(&ingest, &graph, &interner, page);
    assert_eq!(rep.groups.len(), 2);
    let window = rep.page_window().expect("seams report is paged");
    assert_eq!((window.shown, window.total, window.begin), (1, 2, 1));
    assert!(!window.has_more);
}

#[test]
fn test_find_integration_seams_budget_fits_groups() {
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
    let roomy = PageRequest {
        limit: None,
        offset: None,
        budget: Some(usize::MAX),
    };
    let rep = find_integration_seams(&ingest, &graph, &interner, roomy);
    let window = rep.page_window().expect("seams report is paged");
    assert_eq!((window.shown, window.total, window.has_more), (2, 2, false));
    let tight = PageRequest {
        budget: Some(1),
        ..roomy
    };
    let rep = find_integration_seams(&ingest, &graph, &interner, tight);
    let window = rep.page_window().expect("seams report is paged");
    assert_eq!((window.shown, window.total, window.has_more), (1, 2, true));
}

#[test]
fn test_seam_keeps_path_qualified_call_from_shared_name_caller() {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[
            (
                "src/alpha/run.rs",
                Lang::Rust,
                b"pub fn execute() -> u32 {\n    crate::beta::helper()\n}\n",
            ),
            (
                "src/beta/tool.rs",
                Lang::Rust,
                b"pub fn helper() -> u32 {\n    1\n}\n",
            ),
        ],
        &reg,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    let rep = find_integration_seams(&ingest, &graph, &interner, PageRequest::default());
    assert_eq!(rep.groups.len(), 1);
    assert_eq!(rep.groups[0].from_dir, "src/alpha");
    assert_eq!(rep.groups[0].to_dir, "src/beta");
    assert_eq!(rep.groups[0].edges.len(), 1);
    assert_eq!(rep.groups[0].edges[0].caller, "execute");
    assert_eq!(rep.groups[0].edges[0].callee, "helper");
}

#[test]
fn test_seam_ignores_path_qualified_call_within_one_directory() {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[
            (
                "src/alpha/run.rs",
                Lang::Rust,
                b"pub fn execute() -> u32 {\n    crate::alpha::local()\n}\n",
            ),
            (
                "src/alpha/local.rs",
                Lang::Rust,
                b"pub fn local() -> u32 {\n    1\n}\n",
            ),
        ],
        &reg,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    let rep = find_integration_seams(&ingest, &graph, &interner, PageRequest::default());
    assert!(rep.groups.is_empty());
}
