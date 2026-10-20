use ripwire_rs::{
    CorpusView, FileInfo, IngestResult, Interner, Lang, NO_NODE, NodeId, RefRole, Reference,
    SymKind, Symbol,
};

#[test]
fn test_domain_tags() {
    assert_eq!(SymKind::Function.tag(), "fn");
    assert_eq!(RefRole::Call.tag(), "call");
    assert_eq!(Lang::Rust.tag(), "rs");
    assert_eq!(Lang::ObjC.tag(), "m");
    assert!(Lang::from_extension("rs") == Lang::Rust);
    assert!(Lang::from_extension("m") == Lang::ObjC);
    assert!(Lang::from_extension("mm") == Lang::ObjC);
    assert!(Lang::from_extension("unknown") == Lang::Unknown);
    assert!(Lang::C.is_compatible(Lang::ObjC));
    assert!(!Lang::Rust.is_compatible(Lang::Python));
}

#[test]
fn test_interner() {
    let interner = Interner::new();
    let s1 = interner.intern("foo");
    let s2 = interner.intern("bar");
    assert_eq!(s1, interner.intern("foo"));
    assert_eq!(interner.resolve(&s1), "foo");
    assert_eq!(&interner[s1], "foo");
    assert_eq!(&interner[&s2], "bar");
}

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

#[test]
fn test_corpus_view_and_symbol_predicates() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern("src/engine.rs"),
        lang: Lang::Rust,
        line_count: 20,
        byte_size: 200,
    });
    let mut sym = Symbol::new(
        0,
        interner.intern("execute_turn"),
        interner.intern("src/engine.rs::Engine::execute_turn"),
        SymKind::Method,
        Lang::Rust,
        0,
        10,
    )
    .with_scope(interner.intern("Engine"));
    sym.sig_start_byte = 10;
    sym.sig_end_byte = 30;
    sym.end_byte = 90;
    sym.loc = 5;
    ingest.symbols.push(sym);

    let view = (&ingest, &interner);
    assert!(sym.is_callable());
    assert!(sym.is_code_symbol());
    assert!(sym.has_body());
    assert_eq!(sym.end_line(), 14);
    assert_eq!(view.sym_name(&sym), "execute_turn");
    assert_eq!(view.sym_scope(&sym), Some("Engine"));
    assert_eq!(view.sym_path(&sym), "src/engine.rs");
    assert!(view.is_production_symbol(&sym));
    assert_eq!(Lang::from_extension("h"), Lang::Cpp);

    ingest.files.push(FileInfo {
        id: 1,
        path: interner.intern("src/main.rs"),
        lang: Lang::Rust,
        line_count: 15,
        byte_size: 120,
    });
    ingest.references.push(Reference::new(
        NO_NODE,
        interner.intern("crate::engine::Engine"),
        1,
        1,
        Lang::Rust,
        RefRole::Import,
    ));
    ingest.references.push(Reference::new(
        0,
        interner.intern("execute_turn"),
        1,
        8,
        Lang::Rust,
        RefRole::Call,
    ));
    let view = (&ingest, &interner);
    assert_eq!(view.resolve_symbol_importers(&[0]), vec!["src/main.rs"]);
    assert_eq!(
        view.caller_site_locations(0, &[interner.intern("execute_turn")]),
        vec!["src/main.rs:8".to_string()]
    );
    let qual_ref = Reference::new(0, interner.intern("lock"), 1, 9, Lang::Rust, RefRole::Call)
        .with_qualifier(interner.intern("TokenVault"));
    assert!(view.reference_matches_query(&qual_ref, "TokenVault"));
    assert!(view.reference_matches_query(&qual_ref, "TokenVault::lock"));
    assert!(view.reference_matches_query(&qual_ref, "TokenVault.lock"));
    assert!(!view.reference_matches_query(&qual_ref, "OtherVault::lock"));
}

#[test]
fn test_symbol_extract_signature_and_source_text() {
    let interner = Interner::new();
    let mut sym = Symbol::new(
        0,
        interner.intern("resolveRefSpec"),
        interner.intern("src/q.h::resolveRefSpec"),
        SymKind::Function,
        Lang::Cpp,
        0,
        1,
    );
    let src =
        b"inline RefSpec resolveRefSpec(const std::string& root, int a[2]) {\n    return {};\n}";
    sym.sig_start_byte = 0;
    sym.sig_end_byte = src.len() as u32;
    sym.end_byte = src.len() as u32;
    assert_eq!(
        sym.extract_signature(src, "resolveRefSpec"),
        "inline RefSpec resolveRefSpec(const std::string& root, int a[2])"
    );
    assert!(sym.source_text(src).unwrap().contains("return {};"));
    assert_eq!(sym.extract_signature(b"", "fallback"), "fallback");
}

#[test]
fn test_importers_resolve_relative_reexports_and_ignore_call_only_files() {
    let interner = Interner::new();
    let registry = ripwire_rs::LanguageRegistry::new().unwrap();
    let ingest = ripwire_rs::ingest_in_memory_files(
        &[
            ("src/navigate/xml.rs", Lang::Rust, b"pub fn render() {}\n"),
            (
                "src/navigate/mod.rs",
                Lang::Rust,
                b"pub mod xml;\npub use xml::*;\n",
            ),
            (
                "src/lib.rs",
                Lang::Rust,
                b"pub mod navigate;\npub use navigate::render;\n",
            ),
            ("src/cli.rs", Lang::Rust, b"fn run() { render(); }\n"),
        ],
        &registry,
        &interner,
    );
    let render = ingest
        .symbols
        .iter()
        .find(|s| interner.resolve(&s.name) == "render")
        .unwrap()
        .id;
    let view = (&ingest, &interner);
    assert_eq!(
        view.resolve_symbol_importers(&[render]),
        vec!["src/lib.rs", "src/navigate/mod.rs"]
    );
}
