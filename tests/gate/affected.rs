use ripwire_rs::{
    FileInfo, Graph, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol,
    ToXmlString, find_affected_tests, is_test_path,
};

#[test]
fn test_is_test_path_detection() {
    assert!(is_test_path("tests/foo.rs"));
    assert!(is_test_path("src/lib_test.go"));
    assert!(is_test_path("ui/component.test.ts"));
    assert!(is_test_path("core/test_engine.rs"));
    assert!(!is_test_path("src/lib.rs"));
    assert!(!is_test_path("src/engine.rs"));
}

#[test]
fn test_affected_transitive_reachability() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();

    let p_core = interner.intern("src/core.rs");
    let p_service = interner.intern("src/service.rs");
    let p_test = interner.intern("tests/test_service.rs");

    ingest.files.push(FileInfo {
        id: 0,
        path: p_core,
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    });
    ingest.files.push(FileInfo {
        id: 1,
        path: p_service,
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    });
    ingest.files.push(FileInfo {
        id: 2,
        path: p_test,
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    });

    let s0 = Symbol::new(
        0,
        interner.intern("core_fn"),
        interner.intern("core_fn"),
        SymKind::Function,
        Lang::Rust,
        0,
        5,
    );
    let s1 = Symbol::new(
        1,
        interner.intern("service_fn"),
        interner.intern("service_fn"),
        SymKind::Function,
        Lang::Rust,
        1,
        10,
    );
    let s2 = Symbol::new(
        2,
        interner.intern("test_fn"),
        interner.intern("test_fn"),
        SymKind::Function,
        Lang::Rust,
        2,
        15,
    );

    ingest.symbols.push(s0);
    ingest.symbols.push(s1);
    ingest.symbols.push(s2);

    ingest.references.push(Reference::new(
        1,
        interner.intern("core_fn"),
        1,
        12,
        Lang::Rust,
        RefRole::Call,
    ));
    ingest.references.push(Reference::new(
        2,
        interner.intern("service_fn"),
        2,
        18,
        Lang::Rust,
        RefRole::Call,
    ));

    let graph = Graph::build(&ingest, &interner);

    let report = find_affected_tests(&["src/core.rs"], &ingest, &graph, &interner);
    assert_eq!(report.seed_count, 1);
    assert_eq!(report.total_tests, 1);
    assert_eq!(report.tests[0].path, "tests/test_service.rs");
    assert_eq!(report.tests[0].hops, 2);

    let xml = report.to_xml_string();
    assert!(xml.contains("<affected schema=\"ripwire-rs.affected/v1\""));
    assert!(xml.contains("<test p=\"tests/test_service.rs\" hops=\"2\"/>"));
}
