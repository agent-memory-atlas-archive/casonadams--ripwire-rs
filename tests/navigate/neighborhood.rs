use ripwire_rs::navigate::{
    find_symbols_by_name, navigate_1hop, serialize_callers_xml,
    serialize_impact_with_importers_xml, transitive_impact,
};
use ripwire_rs::serialize::GraphGauges;
use ripwire_rs::{
    FileInfo, Graph, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol,
};

#[test]
fn test_navigate_1hop_and_impact() {
    let interner = Interner::new();
    let f0 = FileInfo {
        id: 0,
        path: interner.intern("src/lib.rs"),
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 500,
    };

    let sym_a = Symbol::new(
        0,
        interner.intern("a"),
        interner.intern("c_a"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    );
    let sym_b = Symbol::new(
        1,
        interner.intern("b"),
        interner.intern("c_b"),
        SymKind::Function,
        Lang::Rust,
        0,
        5,
    );
    let sym_c = Symbol::new(
        2,
        interner.intern("c"),
        interner.intern("c_c"),
        SymKind::Function,
        Lang::Rust,
        0,
        10,
    );

    let ref_ab = Reference::new(0, interner.intern("b"), 0, 2, Lang::Rust, RefRole::Call);
    let ref_bc = Reference::new(1, interner.intern("c"), 0, 6, Lang::Rust, RefRole::Call);

    let mut ingest = IngestResult::new();
    ingest.files.push(f0);
    ingest.symbols.extend_from_slice(&[sym_a, sym_b, sym_c]);
    ingest.references.extend_from_slice(&[ref_ab, ref_bc]);

    let graph = Graph::build(&ingest, &interner);
    let ranks = vec![0.2, 0.3, 0.5];

    let matched = find_symbols_by_name("b", &ingest, &interner);
    assert_eq!(matched, vec![1]);

    let nav = navigate_1hop(1, &ingest, &graph, &ranks, &interner).unwrap();
    assert_eq!(nav.target.name, "b");
    assert_eq!(nav.callers.len(), 1);
    assert_eq!(nav.callers[0].name, "a");
    assert_eq!(nav.callers[0].sites, vec!["src/lib.rs:2".to_string()]);
    assert_eq!(nav.callees.len(), 1);
    assert_eq!(nav.callees[0].name, "c");

    let mut callers_xml = Vec::new();
    serialize_callers_xml("b", &nav.callers, &mut callers_xml).unwrap();
    assert!(
        String::from_utf8(callers_xml)
            .unwrap()
            .contains("sites_at=\"src/lib.rs:2\"")
    );

    let impact = transitive_impact(2, &ingest, &graph, &ranks, &interner, None);
    assert_eq!(impact.len(), 2);
    assert_eq!(impact[0].name, "b");
    assert_eq!(impact[0].hops, 1);
    assert_eq!(impact[1].name, "a");
    assert_eq!(impact[1].hops, 2);

    let mut imp_xml = Vec::new();
    serialize_impact_with_importers_xml(
        ".",
        "c",
        &impact,
        vec!["src/main.rs"],
        GraphGauges::ZERO_XML_ATTRS,
        1,
        &mut imp_xml,
    )
    .unwrap();
    let imp_str = String::from_utf8(imp_xml).unwrap();
    assert!(imp_str.contains("importers=\"2\""));
    assert!(imp_str.contains("p=\"src/main.rs\""));
}
