use ripwire_rs::gate::arch::{ArchConfig, check_architecture_rules};
use ripwire_rs::{
    FileInfo, Graph, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol,
};

#[test]
fn test_arch_config_parsing() {
    let rules = r#"
# Sample layering rules
layer domain = src/domain/**
layer cli = src/cli/*
deny domain -> cli
allow cli -> domain
"#;
    let cfg = ArchConfig::parse(rules);
    assert_eq!(cfg.layers.len(), 2);
    assert_eq!(cfg.rules.len(), 2);
    assert_eq!(cfg.assign_layer("src/domain.rs"), Some("domain"));
    assert_eq!(cfg.assign_layer("src/cli/args.rs"), Some("cli"));
}

#[test]
fn test_check_edge_violation() {
    let cfg = ArchConfig::parse(
        "layer cli = src/cli/*\nlayer domain = src/domain*\ndeny cli -> domain\n",
    );
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern("src/cli/args.rs"),
        lang: Lang::Rust,
        line_count: 10,
        byte_size: 100,
    });
    ingest.files.push(FileInfo {
        id: 1,
        path: interner.intern("src/domain.rs"),
        lang: Lang::Rust,
        line_count: 10,
        byte_size: 100,
    });
    ingest.symbols.push(Symbol::new(
        0,
        interner.intern("parse"),
        interner.intern("parse"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    ));
    ingest.symbols.push(Symbol::new(
        1,
        interner.intern("intern"),
        interner.intern("intern"),
        SymKind::Function,
        Lang::Rust,
        1,
        1,
    ));
    ingest.references.push(Reference::new(
        0,
        interner.intern("intern"),
        0,
        2,
        Lang::Rust,
        RefRole::Call,
    ));
    let graph = Graph::build(&ingest, &interner);
    let viols = check_architecture_rules(&cfg, &ingest, &graph, &interner);
    assert_eq!(viols.len(), 1);
    assert_eq!(viols[0].from_layer, "cli");
    assert_eq!(viols[0].to_layer, "domain");
}
