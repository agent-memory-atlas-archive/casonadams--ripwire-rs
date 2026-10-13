use ripwire_rs::{
    CsrMatrix, FileInfo, Graph, IngestResult, Interner, Lang, PageRankConfig, RefRole, Reference,
    SymKind, Symbol, biased_teleport, page_rank, uniform_teleport,
};

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

    // ── 1. TWO-NODE ANALYTICAL SINK GRAPH ────────────────────────────────────
    // Graph: 0 -> 1 (where 1 is a dangling sink with zero out-edges).
    // Uniform prior: p = [0.5, 0.5], alpha = 0.85
    // Analytical derivation:
    //   D = r1
    //   r0 = (alpha * D + 1 - alpha) * p0 = (alpha * r1 + 1 - alpha) * 0.5
    //   r1 = alpha * r0 + (alpha * r1 + 1 - alpha) * 0.5 = alpha * r0 + r0 = (1 + alpha) * r0
    //   r0 + r1 = 1  =>  r0 * (1 + 1 + alpha) = 1  =>  r0 = 1 / (2 + alpha)
    //   r1 = (1 + alpha) / (2 + alpha)
    // With alpha = 0.85:
    //   r0 = 1 / 2.85 ≈ 0.3508771929824561
    //   r1 = 1.85 / 2.85 ≈ 0.6491228070175439
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

    // Sum of probability vector must equal 1.0
    let sum_2node: f64 = res_2node.ranks.iter().sum();
    assert!((sum_2node - 1.0).abs() < 1e-9);

    // ── 2. THREE-NODE ANALYTICAL SINK LINE GRAPH ─────────────────────────────
    // Graph: 0 -> 1 -> 2 (where 2 is a dangling sink).
    // Uniform prior: p = [1/3, 1/3, 1/3], alpha = 0.85
    // Analytical derivation:
    //   D = r2
    //   r0 = (alpha * D + 1 - alpha) * (1/3)
    //   r1 = alpha * r0 + r0 = (1 + alpha) * r0
    //   r2 = alpha * r1 + r0 = (1 + alpha + alpha^2) * r0
    //   r0 + r1 + r2 = 1  =>  r0 * (3 + 2*alpha + alpha^2) = 1
    // Denominator = 3 + 2*(0.85) + 0.85^2 = 3 + 1.70 + 0.7225 = 5.4225
    //   r0 = 1 / 5.4225 ≈ 0.1844167819
    //   r1 = 1.85 / 5.4225 ≈ 0.3411710466
    //   r2 = 2.5725 / 5.4225 ≈ 0.4744121715
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

    // ── 3. THREE-NODE CYCLE GRAPH ────────────────────────────────────────────
    // Graph: 0 -> 1 -> 2 -> 0.
    // By symmetry, all nodes must have rank exactly 1/3 ≈ 0.33333333.
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

    // ── 4. TRUNCATED ITERATIONS & HONEST DISCLOSURE ──────────────────────────
    let trunc_config = PageRankConfig {
        alpha: 0.85,
        tolerance: 1e-15,
        max_iterations: 2,
    };
    let res_trunc = graph_2node.page_rank(&trunc_config);
    assert_eq!(res_trunc.iterations, 2);
    assert!(!res_trunc.converged);

    // ── 5. BIASED TELEPORT PRIOR (PERSONALIZATION) ───────────────────────────
    let biased_prior = biased_teleport(&[9.0, 1.0]);
    assert!((biased_prior[0] - 0.9).abs() < 1e-9);
    assert!((biased_prior[1] - 0.1).abs() < 1e-9);

    let res_biased = graph_2node.page_rank_custom(&config, &biased_prior);
    assert!(res_biased.converged);
    // When teleport is heavily biased to node 0, node 0 has higher rank than with uniform prior
    assert!(res_biased.ranks[0] > res_2node.ranks[0]);

    // ── 6. 1024-BLOCK REDUCTION AND LARGE GRAPH DETERMINISM ──────────────────
    // Construct a synthetic CSR graph with 1200 nodes (> 1024 block reduction size).
    // Node i calls Node (i + 1) % 1200 (a large 1200-cycle).
    let n = 1200;
    let mut row_offsets = vec![0u32; n + 1];
    for i in 0..n {
        row_offsets[i + 1] = (i + 1) as u32;
    }
    // In-edge: target t has caller (t + n - 1) % n
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
    // Bit-identical run-to-run determinism across 1024-block reduction boundaries
    assert_eq!(res1.ranks, res2.ranks);
    // Every node in a uniform cycle has rank 1/N
    let expected_cycle_rank = 1.0 / (n as f64);
    for r in &res1.ranks {
        assert!((r - expected_cycle_rank).abs() < 1e-5);
    }
}
