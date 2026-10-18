use ripwire_rs::{
    CsrMatrix, FileInfo, Graph, IngestResult, Interner, Lang, PageRankConfig, RefRole, Reference,
    SymKind, Symbol, biased_teleport, page_rank, uniform_teleport,
};

#[test]
fn test_uniform_and_biased_teleport() {
    let u = uniform_teleport(4);
    assert_eq!(u.len(), 4);
    assert!((u.iter().sum::<f64>() - 1.0).abs() < 1e-9);

    let biased = biased_teleport(&[1.0, 2.0, 1.0]);
    assert_eq!(biased.len(), 3);
    assert!((biased[0] - 0.25).abs() < 1e-9);
    assert!((biased[1] - 0.50).abs() < 1e-9);
    assert!((biased[2] - 0.25).abs() < 1e-9);

    let empty = uniform_teleport(0);
    assert!(empty.is_empty());
}

#[test]
fn test_empty_graph_pagerank() {
    let empty_csr = CsrMatrix::empty(0, 0);
    let res = page_rank(&empty_csr, &[], &[], &PageRankConfig::default());
    assert_eq!(res.iterations, 0);
    assert!(res.converged);
    assert!(res.ranks.is_empty());
}

#[test]
fn test_truncated_iterations() {
    let csr = CsrMatrix::new(2, 2, vec![0, 0, 1], vec![0], vec![1.0]);
    let w_out = vec![1.0, 0.0];
    let teleport = uniform_teleport(2);
    let cfg = PageRankConfig {
        max_iterations: 1,
        tolerance: 1e-12,
        alpha: 0.85,
    };
    let res = page_rank(&csr, &w_out, &teleport, &cfg);
    assert_eq!(res.iterations, 1);
    assert!(!res.converged);
}

#[test]
fn test_pagerank_numerical_accuracy() {
    let interner = Interner::new();
    let file = FileInfo {
        id: 0,
        path: interner.intern("src/test.rs"),
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 500,
    };

    let sym0 = Symbol::new(
        0,
        interner.intern("n0"),
        interner.intern("c0"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    );
    let sym1 = Symbol::new(
        1,
        interner.intern("n1"),
        interner.intern("c1"),
        SymKind::Function,
        Lang::Rust,
        0,
        2,
    );
    let ref01 = Reference::new(0, interner.intern("n1"), 0, 1, Lang::Rust, RefRole::Call);

    let mut ingest_2node = IngestResult::new();
    ingest_2node.files.push(file);
    ingest_2node.symbols.extend_from_slice(&[sym0, sym1]);
    ingest_2node.references.push(ref01);

    let graph_2node = Graph::build(&ingest_2node, &interner);
    assert_eq!(graph_2node.node_count(), 2);
    assert!(graph_2node.is_dangling(1));

    let config = PageRankConfig::default();
    let res_2node = graph_2node.page_rank(&config);

    assert!(res_2node.converged);
    assert!(res_2node.iterations > 0 && res_2node.iterations <= 100);

    let expected_r0 = 1.0 / 2.85;
    let expected_r1 = 1.85 / 2.85;
    assert!(
        (res_2node.ranks[0] - expected_r0).abs() < 1e-5,
        "got {}, expected {}",
        res_2node.ranks[0],
        expected_r0
    );
    assert!(
        (res_2node.ranks[1] - expected_r1).abs() < 1e-5,
        "got {}, expected {}",
        res_2node.ranks[1],
        expected_r1
    );

    let sum_2node: f64 = res_2node.ranks.iter().sum();
    assert!((sum_2node - 1.0).abs() < 1e-9);

    let sym2 = Symbol::new(
        2,
        interner.intern("n2"),
        interner.intern("c2"),
        SymKind::Function,
        Lang::Rust,
        0,
        3,
    );
    let ref12 = Reference::new(1, interner.intern("n2"), 0, 2, Lang::Rust, RefRole::Call);

    let mut ingest_3node = IngestResult::new();
    ingest_3node.files.push(file);
    ingest_3node.symbols.extend_from_slice(&[sym0, sym1, sym2]);
    ingest_3node.references.extend_from_slice(&[ref01, ref12]);

    let graph_3node = Graph::build(&ingest_3node, &interner);
    assert_eq!(graph_3node.node_count(), 3);
    assert!(graph_3node.is_dangling(2));

    let res_3node = graph_3node.page_rank(&config);
    assert!(res_3node.converged);

    let denom_3node = 5.4225;
    let expected_3node_r0 = 1.0 / denom_3node;
    let expected_3node_r1 = 1.85 / denom_3node;
    let expected_3node_r2 = 2.5725 / denom_3node;

    assert!(
        (res_3node.ranks[0] - expected_3node_r0).abs() < 1e-5,
        "got {}, expected {}",
        res_3node.ranks[0],
        expected_3node_r0
    );
    assert!(
        (res_3node.ranks[1] - expected_3node_r1).abs() < 1e-5,
        "got {}, expected {}",
        res_3node.ranks[1],
        expected_3node_r1
    );
    assert!(
        (res_3node.ranks[2] - expected_3node_r2).abs() < 1e-5,
        "got {}, expected {}",
        res_3node.ranks[2],
        expected_3node_r2
    );

    let sum_3node: f64 = res_3node.ranks.iter().sum();
    assert!((sum_3node - 1.0).abs() < 1e-9);

    let ref20 = Reference::new(2, interner.intern("n0"), 0, 3, Lang::Rust, RefRole::Call);
    let mut ingest_cycle = IngestResult::new();
    ingest_cycle.files.push(file);
    ingest_cycle.symbols.extend_from_slice(&[sym0, sym1, sym2]);
    ingest_cycle
        .references
        .extend_from_slice(&[ref01, ref12, ref20]);

    let graph_cycle = Graph::build(&ingest_cycle, &interner);
    let res_cycle = graph_cycle.page_rank(&config);

    assert!(res_cycle.converged);
    for i in 0..3 {
        assert!(
            (res_cycle.ranks[i] - 1.0 / 3.0).abs() < 1e-5,
            "node {} got {}, expected 1/3",
            i,
            res_cycle.ranks[i]
        );
    }

    let trunc_config = PageRankConfig {
        alpha: 0.85,
        tolerance: 1e-15,
        max_iterations: 2,
    };
    let res_trunc = graph_2node.page_rank(&trunc_config);
    assert_eq!(res_trunc.iterations, 2);
    assert!(!res_trunc.converged);

    let biased_prior = biased_teleport(&[9.0, 1.0]);
    assert!((biased_prior[0] - 0.9).abs() < 1e-9);
    assert!((biased_prior[1] - 0.1).abs() < 1e-9);

    let res_biased = graph_2node.page_rank_custom(&config, &biased_prior);
    assert!(res_biased.converged);
    assert!(res_biased.ranks[0] > res_2node.ranks[0]);

    let n = 1200;
    let mut row_offsets = vec![0u32; n + 1];
    for i in 0..n {
        row_offsets[i + 1] = (i + 1) as u32;
    }
    let col_indices: Vec<u32> = (0..n as u32)
        .map(|t| (t + n as u32 - 1) % n as u32)
        .collect();
    let values = vec![1.0f32; n];
    let w_out = vec![1.0f64; n];

    let large_csr = CsrMatrix::new(n, n, row_offsets, col_indices, values);
    let teleport = uniform_teleport(n);

    let res1 = page_rank(&large_csr, &w_out, &teleport, &config);
    let res2 = page_rank(&large_csr, &w_out, &teleport, &config);

    assert!(res1.converged);
    assert_eq!(res1.iterations, res2.iterations);
    assert_eq!(res1.ranks, res2.ranks);
    let expected_cycle_rank = 1.0 / (n as f64);
    for r in &res1.ranks {
        assert!((r - expected_cycle_rank).abs() < 1e-5);
    }
}
