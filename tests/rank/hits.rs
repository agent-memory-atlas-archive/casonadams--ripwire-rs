use ripwire_rs::{
    EdgeAcc, EdgeProv, Graph, HitsConfig, IngestResult, Interner, Lang, ResolutionResult, SymKind,
    Symbol, compute_hits,
};
use std::collections::HashMap;

#[test]
fn test_hits_empty_graph() {
    let interner = Interner::new();
    let ingest = IngestResult::new();
    let graph = Graph::build(&ingest, &interner);
    let res = compute_hits(&graph, &HitsConfig::default());
    assert_eq!(res.authority.len(), 0);
    assert_eq!(res.hub.len(), 0);
    assert!(res.converged);
}

#[test]
fn test_hits_two_node_chain() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    let f = interner.intern("test.rs");
    let n0 = interner.intern("caller");
    let n1 = interner.intern("callee");

    ingest
        .symbols
        .push(Symbol::new(0, f, n0, SymKind::Function, Lang::Rust, 0, 1));
    ingest
        .symbols
        .push(Symbol::new(1, f, n1, SymKind::Function, Lang::Rust, 0, 10));

    let resolution = ResolutionResult {
        edge_accumulators: HashMap::from([(
            (0, 1),
            EdgeAcc {
                n_ref: 1,
                conf_sum: 1.0,
                prov: EdgeProv::Local,
            },
        )]),
        amb_out: vec![0, 0],
        unresolved: 0,
        external: 0,
        declined: 0,
    };
    let graph = Graph::from_resolution(2, resolution);
    let res = compute_hits(&graph, &HitsConfig::default());

    assert_eq!(res.authority.len(), 2);
    assert_eq!(res.hub.len(), 2);
    assert!(res.converged);
    assert!(res.authority[1] > res.authority[0]);
    assert!(res.hub[0] > res.hub[1]);
}
