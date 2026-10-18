use ripwire_rs::metrics::{
    classify_identifier_casing, compute_comment_coherence, compute_context_ratio, compute_ensemble,
    serialize_executive_report_md, serialize_naming_calibration_xml,
    serialize_naming_consistency_xml,
};
use ripwire_rs::{FileInfo, Graph, IngestResult, Interner, Lang, SymKind, Symbol};

#[test]
fn test_context_ratio_and_ensemble() {
    let ingest = IngestResult::new();
    let interner = Interner::new();
    let graph = Graph::build(&ingest, &interner);

    let cr = compute_context_ratio(&ingest, &graph, &interner);
    assert_eq!(cr.units, 0);

    let ens = compute_ensemble(&ingest, &graph, &interner);
    assert_eq!(ens.eligible, 0);
    let cc = compute_comment_coherence(&ingest, &interner);
    assert_eq!(cc.documented, 0);
}

#[test]
fn test_evaluate_symbol_ensemble() {
    let interner = Interner::new();
    let mut sym = Symbol::new(
        0,
        interner.intern("complex_fn"),
        interner.intern("src/lib.rs::complex_fn"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    );
    sym.ccx = 20;
    sym.loc = 100;
    sym.max_nest = 5;
    sym.cx = 15;
    sym.params = 5;
    let mut ingest = IngestResult::new();
    ingest.symbols.push(sym);
    let graph = Graph::build(&ingest, &interner);
    let ens = compute_ensemble(&ingest, &graph, &interner);
    assert_eq!(ens.rows.len(), 1);
    assert!(ens.rows[0].fired.contains("structural"));
    assert!(ens.rows[0].fired.contains("historical"));
    assert!(ens.rows[0].fired.contains("lexical"));
}

#[test]
fn test_naming_and_executive_report() {
    assert_eq!(
        classify_identifier_casing("MAX_SIZE"),
        "SCREAMING_SNAKE_CASE"
    );
    assert_eq!(classify_identifier_casing("my_func"), "snake_case");
    assert_eq!(classify_identifier_casing("MyClass"), "PascalCase");
    assert_eq!(classify_identifier_casing("myVar"), "camelCase");
    assert_eq!(classify_identifier_casing("run"), "lower");
    assert_eq!(classify_identifier_casing("___"), "other");

    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    let name_spur = interner.intern("my_func");
    let path_spur = interner.intern("src/lib.rs");
    ingest.files.push(FileInfo {
        id: 0,
        path: path_spur,
        lang: Lang::Rust,
        line_count: 10,
        byte_size: 100,
    });
    ingest.symbols.push(Symbol::new(
        0,
        name_spur,
        name_spur,
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    ));
    let graph = Graph::build(&ingest, &interner);
    let mut buf = Vec::new();
    serialize_naming_consistency_xml(".", false, &ingest, &interner, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("snake_case"));
    buf.clear();
    serialize_naming_calibration_xml(".", &ingest, &interner, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("ripwire.naming-calibration/v1"));
    buf.clear();
    serialize_executive_report_md(".", &ingest, &graph, &[1.0], &interner, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("# Architecture & Quality Report"));
}
