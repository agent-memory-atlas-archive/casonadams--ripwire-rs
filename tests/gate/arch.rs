use ripwire_rs::gate::arch::{ArchConfig, check_architecture_rules};
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, ingest_in_memory_files};

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
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[
            (
                "src/cli/args.rs",
                Lang::Rust,
                b"pub fn parse() { intern(); }\n",
            ),
            ("src/domain.rs", Lang::Rust, b"pub fn intern() {}\n"),
        ],
        &reg,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    let viols = check_architecture_rules(&cfg, &ingest, &graph, &interner);
    assert_eq!(viols.len(), 1);
    assert_eq!(viols[0].from_layer, "cli");
    assert_eq!(viols[0].to_layer, "domain");
}
