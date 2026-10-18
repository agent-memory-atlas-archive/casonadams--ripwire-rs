use ripwire_rs::graph::community::{detect_communities, detect_hierarchy};
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, ingest_in_memory_files};
use std::collections::HashSet;

#[test]
fn test_community_detection_two_cliques() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();

    let code1 = b"fn a() { b(); c(); } fn b() { a(); c(); } fn c() { a(); b(); }";
    let code2 = b"fn x() { y(); z(); } fn y() { x(); z(); } fn z() { x(); y(); }";

    let ingest = ingest_in_memory_files(
        &[
            ("src/cluster1.rs", Lang::Rust, code1),
            ("src/cluster2.rs", Lang::Rust, code2),
        ],
        &registry,
        &interner,
    );

    let graph = Graph::build(&ingest, &interner);
    let comms = detect_communities(&graph);

    assert_eq!(comms.comm.len(), 6);
    assert_eq!(comms.count, 2);
    assert_eq!(comms.comm[0], comms.comm[1]);
    assert_eq!(comms.comm[1], comms.comm[2]);
    assert_eq!(comms.comm[3], comms.comm[4]);
    assert_eq!(comms.comm[4], comms.comm[5]);
    assert_ne!(comms.comm[0], comms.comm[3]);
}

#[test]
fn test_hierarchy_merges_linked_cliques_at_a_coarser_level() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let clique = |p: usize| {
        let link = if p.is_multiple_of(2) {
            let q = p + 1;
            format!("q{q}a(); q{q}b(); q{q}c();")
        } else {
            String::new()
        };
        format!(
            "fn q{p}a() {{ q{p}b(); q{p}c(); {link} }} fn q{p}b() {{ q{p}a(); q{p}c(); {link} }} fn q{p}c() {{ q{p}a(); q{p}b(); {link} }}"
        )
    };
    let files: Vec<(String, String)> = (0..8)
        .map(|p| (format!("src/q{p}.rs"), clique(p)))
        .collect();
    let mem: Vec<(&str, Lang, &[u8])> = files
        .iter()
        .map(|(p, c)| (p.as_str(), Lang::Rust, c.as_bytes()))
        .collect();
    let ingest = ingest_in_memory_files(&mem, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let levels = detect_hierarchy(&graph);

    assert!(levels.len() >= 2);
    let finest = &levels[0];
    let top = levels.last().unwrap();
    assert_eq!(finest.len(), graph.node_count());
    let distinct = |v: &Vec<u32>| v.iter().collect::<HashSet<_>>().len();
    assert!(distinct(top) < distinct(finest));
    let comm_of = |name: &str| {
        let id = ingest
            .symbols
            .iter()
            .position(|s| interner.resolve(&s.name) == name)
            .unwrap();
        (finest[id], top[id])
    };
    assert_eq!(comm_of("q0a").0, comm_of("q0b").0);
    assert_ne!(comm_of("q0a").0, comm_of("q1a").0);
    assert_eq!(comm_of("q0a").1, comm_of("q1a").1);
    assert_ne!(comm_of("q0a").1, comm_of("q2a").1);
}
