use ripwire_rs::gate::seams::find_integration_seams;
use ripwire_rs::{
    FileInfo, Graph, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol,
};

#[test]
fn test_find_integration_seams_across_directories() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern("src/gate/seams.rs"),
        lang: Lang::Rust,
        line_count: 20,
        byte_size: 200,
    });
    ingest.files.push(FileInfo {
        id: 1,
        path: interner.intern("lib.rs"),
        lang: Lang::Rust,
        line_count: 20,
        byte_size: 200,
    });
    ingest.symbols.push(Symbol::new(
        0,
        interner.intern("caller_fn"),
        interner.intern("caller_fn"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    ));
    ingest.symbols.push(Symbol::new(
        1,
        interner.intern("callee_fn"),
        interner.intern("callee_fn"),
        SymKind::Function,
        Lang::Rust,
        1,
        1,
    ));
    ingest.references.push(Reference::new(
        0,
        interner.intern("callee_fn"),
        0,
        5,
        Lang::Rust,
        RefRole::Call,
    ));
    let graph = Graph::build(&ingest, &interner);
    let rep = find_integration_seams(&ingest, &graph, &interner);
    assert_eq!(rep.groups.len(), 1);
    assert_eq!(rep.groups[0].from_dir, "src/gate");
    assert_eq!(rep.groups[0].to_dir, ".");
}
