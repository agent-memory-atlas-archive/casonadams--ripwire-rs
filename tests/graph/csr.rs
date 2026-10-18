use ripwire_rs::{
    CsrMatrix, EdgeAcc, FileInfo, Graph, IngestResult, Interner, Lang, RefRole, Reference,
    ResolutionResult, SymKind, Symbol,
};

#[test]
fn test_csr_matrix_empty_and_verify() {
    let empty = CsrMatrix::empty(4, 4);
    assert_eq!(empty.rows, 4);
    assert_eq!(empty.cols, 4);
    assert_eq!(empty.nnz(), 0);
    assert!(empty.verify());
}

#[test]
fn test_graph_simple_chain() {
    let mut resolution = ResolutionResult {
        amb_out: vec![0, 0, 0],
        ..Default::default()
    };
    resolution.edge_accumulators.insert(
        (0, 1),
        EdgeAcc {
            conf_sum: 1.0,
            n_ref: 1,
            is_split: false,
        },
    );
    resolution.edge_accumulators.insert(
        (1, 2),
        EdgeAcc {
            conf_sum: 1.0,
            n_ref: 1,
            is_split: false,
        },
    );

    let graph = Graph::from_resolution(3, resolution);
    assert_eq!(graph.node_count(), 3);
    assert_eq!(graph.edge_count(), 2);
    assert!(graph.in_edges.verify());

    assert_eq!(graph.out_degree(0), 1);
    assert_eq!(graph.out_degree(1), 1);
    assert_eq!(graph.out_degree(2), 0);
    assert!(graph.is_dangling(2));
    assert!(!graph.is_dangling(0));

    assert_eq!(graph.in_degree(0), 0);
    assert_eq!(graph.in_degree(1), 1);
    assert_eq!(graph.in_degree(2), 1);

    let callers_of_2: Vec<_> = graph.callers(2).collect();
    assert_eq!(callers_of_2, vec![(1, 1.0)]);

    let callees_of_0: Vec<_> = graph.callees(0).collect();
    assert_eq!(callees_of_0, vec![(1, 1.0)]);
}

#[test]
fn test_csr_diamond_and_cycles() {
    let interner = Interner::new();

    let file = FileInfo {
        id: 0,
        path: interner.intern("src/diamond.rs"),
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    };

    let name_a = interner.intern("node_a");
    let name_b = interner.intern("node_b");
    let name_c = interner.intern("node_c");
    let name_d = interner.intern("node_d");
    let canon = interner.intern("canon");

    let node_a = Symbol::new(0, name_a, canon, SymKind::Function, Lang::Rust, 0, 10);
    let node_b = Symbol::new(1, name_b, canon, SymKind::Function, Lang::Rust, 0, 20);
    let node_c = Symbol::new(2, name_c, canon, SymKind::Function, Lang::Rust, 0, 30);
    let node_d = Symbol::new(3, name_d, canon, SymKind::Function, Lang::Rust, 0, 40);

    let ref_a_b = Reference::new(0, name_b, 0, 11, Lang::Rust, RefRole::Call);
    let ref_a_c = Reference::new(0, name_c, 0, 12, Lang::Rust, RefRole::Call);
    let ref_b_d = Reference::new(1, name_d, 0, 21, Lang::Rust, RefRole::Call);
    let ref_c_d = Reference::new(2, name_d, 0, 31, Lang::Rust, RefRole::Call);

    let mut ingest_diamond = IngestResult::new();
    ingest_diamond.files.push(file);
    ingest_diamond
        .symbols
        .extend_from_slice(&[node_a, node_b, node_c, node_d]);
    ingest_diamond
        .references
        .extend_from_slice(&[ref_a_b, ref_a_c, ref_b_d, ref_c_d]);

    let diamond_graph = Graph::build(&ingest_diamond, &interner);

    assert_eq!(diamond_graph.node_count(), 4);
    assert_eq!(diamond_graph.edge_count(), 4);
    assert!(diamond_graph.in_edges.verify());

    assert_eq!(diamond_graph.in_edges.row_offsets, vec![0, 0, 1, 2, 4]);
    assert_eq!(diamond_graph.in_edges.col_indices, vec![0, 0, 1, 2]);

    for &val in &diamond_graph.in_edges.values {
        assert!((val - 1.0).abs() < 1e-6);
    }

    assert_eq!(diamond_graph.out_degree(0), 2);
    assert_eq!(diamond_graph.out_degree(1), 1);
    assert_eq!(diamond_graph.out_degree(2), 1);
    assert_eq!(diamond_graph.out_degree(3), 0);

    assert!(!diamond_graph.is_dangling(0));
    assert!(!diamond_graph.is_dangling(1));
    assert!(!diamond_graph.is_dangling(2));
    assert!(diamond_graph.is_dangling(3));

    assert!((diamond_graph.w_out_deg[0] - 2.0).abs() < 1e-6);
    assert!((diamond_graph.w_out_deg[3] - 0.0).abs() < 1e-6);

    let callers_of_d: Vec<_> = diamond_graph.callers(3).collect();
    assert_eq!(callers_of_d, vec![(1, 1.0), (2, 1.0)]);

    let callees_of_a: Vec<_> = diamond_graph.callees(0).collect();
    assert_eq!(callees_of_a, vec![(1, 1.0), (2, 1.0)]);

    let cycle_a = Symbol::new(0, name_a, canon, SymKind::Function, Lang::Rust, 0, 10);
    let cycle_b = Symbol::new(1, name_b, canon, SymKind::Function, Lang::Rust, 0, 20);
    let cycle_c = Symbol::new(2, name_c, canon, SymKind::Function, Lang::Rust, 0, 30);

    let ref_cycle_ab = Reference::new(0, name_b, 0, 11, Lang::Rust, RefRole::Call);
    let ref_cycle_bc = Reference::new(1, name_c, 0, 21, Lang::Rust, RefRole::Call);
    let ref_cycle_ca = Reference::new(2, name_a, 0, 31, Lang::Rust, RefRole::Call);

    let mut ingest_cycle = IngestResult::new();
    ingest_cycle.files.push(file);
    ingest_cycle
        .symbols
        .extend_from_slice(&[cycle_a, cycle_b, cycle_c]);
    ingest_cycle
        .references
        .extend_from_slice(&[ref_cycle_ab, ref_cycle_bc, ref_cycle_ca]);

    let cycle_graph = Graph::build(&ingest_cycle, &interner);

    assert_eq!(cycle_graph.node_count(), 3);
    assert_eq!(cycle_graph.edge_count(), 3);
    assert!(cycle_graph.in_edges.verify());

    for node in 0..3 {
        assert_eq!(cycle_graph.in_degree(node), 1);
        assert_eq!(cycle_graph.out_degree(node), 1);
        assert!(!cycle_graph.is_dangling(node));
        assert!((cycle_graph.w_out_deg[node as usize] - 1.0).abs() < 1e-6);
    }

    assert_eq!(cycle_graph.in_edges.row_offsets, vec![0, 1, 2, 3]);
    assert_eq!(cycle_graph.in_edges.col_indices, vec![2, 0, 1]);

    let mut ingest_multi = IngestResult::new();
    ingest_multi.files.push(file);
    ingest_multi.symbols.extend_from_slice(&[node_a, node_b]);
    ingest_multi.references.extend_from_slice(&[
        Reference::new(0, name_b, 0, 11, Lang::Rust, RefRole::Call),
        Reference::new(0, name_b, 0, 12, Lang::Rust, RefRole::Call),
    ]);

    let multi_graph = Graph::build(&ingest_multi, &interner);
    assert_eq!(multi_graph.edge_count(), 1);
    let expected_w = 2.0f32.sqrt();
    assert!((multi_graph.in_edges.values[0] - expected_w).abs() < 1e-6);

    let name_helper = interner.intern("helper");
    let helper1 = Symbol::new(1, name_helper, canon, SymKind::Function, Lang::Rust, 0, 20);
    let helper2 = Symbol::new(2, name_helper, canon, SymKind::Function, Lang::Rust, 0, 30);
    let ref_amb = Reference::new(0, name_helper, 0, 11, Lang::Rust, RefRole::Call);

    let mut ingest_amb = IngestResult::new();
    ingest_amb.files.push(file);
    ingest_amb
        .symbols
        .extend_from_slice(&[node_a, helper1, helper2]);
    ingest_amb.references.push(ref_amb);

    let amb_graph = Graph::build(&ingest_amb, &interner);
    assert_eq!(amb_graph.edge_count(), 2);
    assert!((amb_graph.in_edges.values[0] - 0.5).abs() < 1e-6);
    assert!((amb_graph.in_edges.values[1] - 0.5).abs() < 1e-6);
    assert_eq!(amb_graph.ambiguity(0), 1);
    assert_eq!(amb_graph.ambiguity(1), 0);
    assert_eq!(amb_graph.ambiguity(2), 0);
}
