use ripwire_rs::metrics::compute_quality_panel_report;
use ripwire_rs::serialize::PageRequest;
use ripwire_rs::{
    FileInfo, Graph, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol, ToXmlString,
};

#[test]
fn test_quality_panel_computation() {
    let ingest = IngestResult::new();
    let interner = Interner::new();
    let graph = Graph::build(&ingest, &interner);
    let rep =
        compute_quality_panel_report(&ingest, &graph, &interner, None, PageRequest::default());
    assert_eq!(rep.preset, "default");
    assert_eq!(rep.cut, 2);

    assert!(
        rep.to_xml_string()
            .contains("schema=\"ripwire-rs.quality-panel/v1\"")
    );
}

#[test]
fn test_check_symbol_families() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern("src/lib.rs"),
        lang: Lang::Rust,
        line_count: 150,
        byte_size: 3000,
    });
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
    sym.params = 6;
    sym.cx = 15;
    let state_sym = Symbol::new(
        1,
        interner.intern("GLOBAL_STATE"),
        interner.intern("src/lib.rs::GLOBAL_STATE"),
        SymKind::Var,
        Lang::Rust,
        0,
        120,
    );
    ingest.symbols.push(sym);
    ingest.symbols.push(state_sym);
    ingest.references.push(Reference::new(
        0,
        interner.intern("GLOBAL_STATE"),
        0,
        10,
        Lang::Rust,
        RefRole::Write,
    ));
    let graph = Graph::build(&ingest, &interner);
    let rep =
        compute_quality_panel_report(&ingest, &graph, &interner, None, PageRequest::default());
    assert_eq!(rep.rows.len(), 1);
    assert!(rep.rows[0].fam >= 3);
    assert!(rep.rows[0].fired.contains("structural"));
    assert!(rep.rows[0].fired.contains("state"));
    assert!(rep.rows[0].fired.contains("historical"));
}
