use rwire::{
    FileInfo, IngestResult, Interner, Lang, NodeId, RefRole, Reference, SymKind, Symbol, NO_NODE,
};

#[test]
fn test_domain_interning() {
    let interner = Interner::new();

    let file_path = interner.intern("src/lib.rs");
    let file_info = FileInfo {
        id: 0,
        path: file_path,
        lang: Lang::Rust,
        line_count: 10,
        byte_size: 256,
    };

    let sym_name = interner.intern("build_graph");
    let canon_id = interner.intern("src/lib.rs::build_graph");
    let symbol = Symbol::new(
        0 as NodeId,
        sym_name,
        canon_id,
        SymKind::Function,
        Lang::Rust,
        0,
        5,
    );

    let callee = interner.intern("parse_ast");
    let reference = Reference::new(0 as NodeId, callee, 0, 7, Lang::Rust, RefRole::Call);

    let mut ingest = IngestResult::new();
    ingest.files.push(file_info);
    ingest.symbols.push(symbol);
    ingest.references.push(reference);

    assert_eq!(ingest.file_count(), 1);
    assert_eq!(ingest.symbol_count(), 1);
    assert_eq!(ingest.reference_count(), 1);

    // Verify interner resolutions
    assert_eq!(interner.resolve(&ingest.files[0].path), "src/lib.rs");
    assert_eq!(interner.resolve(&ingest.symbols[0].name), "build_graph");
    assert_eq!(
        interner.resolve(&ingest.references[0].callee_name),
        "parse_ast"
    );
    assert_eq!(ingest.symbols[0].kind.tag(), "fn");
    assert_eq!(ingest.references[0].role.tag(), "call");
    assert_eq!(ingest.symbols[0].lang.tag(), "rs");
    assert_ne!(ingest.symbols[0].id, NO_NODE);
}
