use ripwire_rs::{
    FileInfo, Graph, IngestResult, Interner, Lang, PageRankResult, RefRole, Reference,
    SerializeOptions, SymKind, Symbol, serialize_xml_to_string,
};

#[test]
fn test_serialize_xml_structure() {
    let interner = Interner::new();
    let f0 = FileInfo {
        id: 0,
        path: interner.intern("src/main.rs"),
        lang: Lang::Rust,
        line_count: 20,
        byte_size: 200,
    };

    let sym_a = Symbol::new(
        0,
        interner.intern("start"),
        interner.intern("c_start"),
        SymKind::Function,
        Lang::Rust,
        0,
        5,
    );
    let sym_b = Symbol::new(
        1,
        interner.intern("run"),
        interner.intern("c_run"),
        SymKind::Function,
        Lang::Rust,
        0,
        10,
    );

    let ref_ab = Reference::new(0, interner.intern("run"), 0, 6, Lang::Rust, RefRole::Call);

    let mut ingest = IngestResult::new();
    ingest.files.push(f0);
    ingest.symbols.extend_from_slice(&[sym_a, sym_b]);
    ingest.references.push(ref_ab);

    let graph = Graph::build(&ingest, &interner);
    let pr = PageRankResult {
        iterations: 12,
        converged: true,
        ranks: vec![0.35, 0.65],
    };

    let xml = serialize_xml_to_string(
        &ingest,
        &graph,
        &pr,
        &interner,
        &SerializeOptions::default(),
    );

    assert!(xml.starts_with("<r schema=\"ripwire-rs.map/v1\" root=\".\" files=\"1\" symbols=\"2\" edges=\"1\" shown=\"2\" ambiguous=\"0\" unresolved=\"0\" pr_iters=\"12\">"));
    assert!(xml.contains("<f p=\"src/main.rs\">"));
    assert!(xml.contains("<s t=\"fn\" n=\"run\" k=\"0.6500\"></s>"));
    assert!(xml.contains("<s t=\"fn\" n=\"start\" k=\"0.3500\"><c n=\"run\"/></s>"));
    assert!(xml.ends_with("</f></r>\n"));
}

#[test]
fn test_serialize_xml_inventory_and_entries() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern("src/main.rs"),
        lang: Lang::Rust,
        line_count: 10,
        byte_size: 100,
    });
    ingest.files.push(FileInfo {
        id: 1,
        path: interner.intern("src/helper.rs"),
        lang: Lang::Rust,
        line_count: 10,
        byte_size: 100,
    });
    ingest.files.push(FileInfo {
        id: 2,
        path: interner.intern("docs/README.md"),
        lang: Lang::Markdown,
        line_count: 10,
        byte_size: 100,
    });
    ingest.symbols.push(Symbol::new(
        0,
        interner.intern("main"),
        interner.intern("src/main.rs::main"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    ));
    ingest.symbols.push(Symbol::new(
        1,
        interner.intern("helper"),
        interner.intern("src/helper.rs::helper"),
        SymKind::Function,
        Lang::Rust,
        1,
        1,
    ));
    let graph = Graph::build(&ingest, &interner);
    let pr = PageRankResult {
        iterations: 5,
        converged: true,
        ranks: vec![0.8, 0.2],
    };
    let xml = serialize_xml_to_string(
        &ingest,
        &graph,
        &pr,
        &interner,
        &SerializeOptions {
            top_k: Some(1),
            emit_legend: true,
            ..Default::default()
        },
    );
    assert!(xml.contains("<inv listed=\"1\" unlisted=\"2\">"));
    assert!(xml.contains("<entry p=\"src/main.rs\" n=\"main\"></entry>"));
    assert!(xml.contains("<ls p=\"src\" n=\"1\" f=\"helper.rs\"/>"));
    assert!(xml.contains("<ls p=\"docs\" n=\"1\"/>"));
    let small_budget_xml = serialize_xml_to_string(
        &ingest,
        &graph,
        &pr,
        &interner,
        &SerializeOptions {
            top_k: Some(1),
            emit_legend: true,
            pack_budget_bytes: Some(2000),
            ..Default::default()
        },
    );
    assert!(!small_budget_xml.contains("<inv "));
}
