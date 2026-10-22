use ripwire_rs::metrics::{
    NamingScope, classify_identifier_casing, compute_comment_coherence,
    compute_comment_coherence_with_sources, compute_context_ratio, compute_ensemble,
    serialize_executive_report_md, serialize_naming_calibration_xml,
    serialize_naming_consistency_xml,
};
use ripwire_rs::serialize::PageRequest;
use ripwire_rs::{FileInfo, Graph, IngestResult, Interner, Lang, SymKind, Symbol};

#[test]
fn test_context_ratio_and_ensemble() {
    let ingest = IngestResult::new();
    let interner = Interner::new();
    let graph = Graph::build(&ingest, &interner);

    let cr = compute_context_ratio(&ingest, &graph, &interner, PageRequest::default());
    assert_eq!(cr.units, 0);

    let ens = compute_ensemble(&ingest, &graph, &interner, PageRequest::default());
    assert_eq!(ens.eligible, 0);
    let cc = compute_comment_coherence(&ingest, &interner, PageRequest::default());
    assert_eq!(cc.documented, 0);
}

#[test]
fn test_evaluate_symbol_ensemble() {
    let interner = Interner::new();
    let mut sym = Symbol::new(
        0,
        interner.intern("complex_fn"),
        interner.intern("src/lib.rs::complex_fn"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    );
    sym.ccx = 20;
    sym.loc = 100;
    sym.max_nest = 5;
    sym.cx = 15;
    sym.params = 5;
    let mut ingest = IngestResult::new();
    ingest.symbols.push(sym);
    let graph = Graph::build(&ingest, &interner);
    let ens = compute_ensemble(&ingest, &graph, &interner, PageRequest::default());
    assert_eq!(ens.rows.len(), 1);
    assert!(ens.rows[0].fired.contains("structural"));
    assert!(ens.rows[0].fired.contains("historical"));
    assert!(ens.rows[0].fired.contains("lexical"));
}

#[test]
fn test_naming_and_executive_report() {
    assert_eq!(
        classify_identifier_casing("MAX_SIZE"),
        "SCREAMING_SNAKE_CASE"
    );
    assert_eq!(classify_identifier_casing("my_func"), "snake_case");
    assert_eq!(classify_identifier_casing("MyClass"), "PascalCase");
    assert_eq!(classify_identifier_casing("myVar"), "camelCase");
    assert_eq!(classify_identifier_casing("run"), "lower");
    assert_eq!(classify_identifier_casing("___"), "other");

    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    let name_spur = interner.intern("my_func");
    let path_spur = interner.intern("src/lib.rs");
    ingest.files.push(FileInfo {
        id: 0,
        path: path_spur,
        lang: Lang::Rust,
        line_count: 10,
        byte_size: 100,
    });
    ingest.symbols.push(Symbol::new(
        0,
        name_spur,
        name_spur,
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    ));
    let graph = Graph::build(&ingest, &interner);
    let mut buf = Vec::new();
    serialize_naming_consistency_xml(
        NamingScope {
            root: ".",
            include_locals: false,
        },
        &ingest,
        &interner,
        PageRequest::default(),
        &mut buf,
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("snake_case"));
    buf.clear();
    serialize_naming_calibration_xml(".", &ingest, &interner, PageRequest::default(), &mut buf)
        .unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("ripwire-rs.naming-calibration/v1"));
    buf.clear();
    serialize_executive_report_md(".", &ingest, &graph, &[1.0], &interner, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("# Architecture & Quality Report"));
}

#[test]
fn test_compute_comment_coherence_with_sources() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();

    // File 1: Rust doc comments with attributes
    let rust_file = interner.intern("src/calc.rs");
    ingest.files.push(FileInfo {
        id: 0,
        path: rust_file,
        lang: Lang::Rust,
        line_count: 20,
        byte_size: 200,
    });
    let sym_rust_name = interner.intern("compute_total");
    let mut sym_rust = Symbol::new(
        0,
        sym_rust_name,
        sym_rust_name,
        SymKind::Function,
        Lang::Rust,
        0,
        4,
    );
    let rust_src = b"/// compute the total sum\n#[inline]\n#[must_use]\n\nfn compute_total() {}\n";
    let rust_needle = b"fn compute_total(";
    sym_rust.sig_start_byte = rust_src
        .windows(rust_needle.len())
        .position(|w| w == rust_needle)
        .unwrap() as u32;
    sym_rust.loc = 5;
    ingest.symbols.push(sym_rust);

    // File 2: Python doc comments with # and decorator
    let py_file = interner.intern("src/metrics.py");
    ingest.files.push(FileInfo {
        id: 1,
        path: py_file,
        lang: Lang::Python,
        line_count: 20,
        byte_size: 200,
    });
    let sym_py_name = interner.intern("calculate_metrics");
    let mut sym_py = Symbol::new(
        1,
        sym_py_name,
        sym_py_name,
        SymKind::Function,
        Lang::Python,
        1,
        4,
    );
    let py_src =
        b"# calculate and export metrics report\n@property\ndef calculate_metrics(): pass\n";
    let py_needle = b"def calculate_metrics(";
    sym_py.sig_start_byte = py_src
        .windows(py_needle.len())
        .position(|w| w == py_needle)
        .unwrap() as u32;
    sym_py.loc = 4;
    ingest.symbols.push(sym_py);

    // File 3: C-style block comments and multi-function boundary check
    let c_file = interner.intern("src/math.c");
    ingest.files.push(FileInfo {
        id: 2,
        path: c_file,
        lang: Lang::C,
        line_count: 30,
        byte_size: 300,
    });
    let sym_c_name = interner.intern("matrix_multiply");
    let mut sym_c = Symbol::new(2, sym_c_name, sym_c_name, SymKind::Function, Lang::C, 2, 6);
    let c_src = b"void previous_step() {}\n\n/*\n * matrix multiply algorithm\n */\nvoid matrix_multiply() {}\n";
    let c_needle = b"void matrix_multiply";
    sym_c.sig_start_byte = c_src
        .windows(c_needle.len())
        .position(|w| w == c_needle)
        .unwrap() as u32;
    sym_c.loc = 6;
    ingest.symbols.push(sym_c);

    // Undocumented function in File 1
    let sym_undoc_name = interner.intern("undocumented_helper");
    let mut sym_undoc = Symbol::new(
        0,
        sym_undoc_name,
        sym_undoc_name,
        SymKind::Function,
        Lang::Rust,
        0,
        10,
    );
    sym_undoc.sig_start_byte = 0;
    sym_undoc.loc = 3;
    ingest.symbols.push(sym_undoc);

    let sources: &[(&str, &[u8])] = &[
        ("src/calc.rs", rust_src),
        ("src/metrics.py", py_src),
        ("src/math.c", c_src),
    ];

    let report =
        compute_comment_coherence_with_sources(sources, &ingest, &interner, PageRequest::default());
    assert_eq!(report.documented, 3);
    assert_eq!(report.no_comment, 1);
    assert_eq!(report.rows.len(), 3);

    // Verify compute_total: ["compute", "the", "total", "sum"] (4 words, 2 restated)
    let rust_row = report
        .rows
        .iter()
        .find(|r| r.name == "compute_total")
        .unwrap();
    assert_eq!(rust_row.words, 4);
    assert_eq!(rust_row.c_terms, 4);
    assert_eq!(rust_row.restate, 2);
    assert!((rust_row.c_coeff - 0.5).abs() < 1e-4);

    // Verify calculate_metrics: ["calculate", "and", "export", "metrics", "report"] (5 words, 2 restated)
    let py_row = report
        .rows
        .iter()
        .find(|r| r.name == "calculate_metrics")
        .unwrap();
    assert_eq!(py_row.words, 5);
    assert_eq!(py_row.c_terms, 5);
    assert_eq!(py_row.restate, 2);
    assert!((py_row.c_coeff - 0.4).abs() < 1e-4);

    // Verify matrix_multiply: boundary stopped at previous_step(), words: ["matrix", "multiply", "algorithm"] (3 words, 2 restated)
    let c_row = report
        .rows
        .iter()
        .find(|r| r.name == "matrix_multiply")
        .unwrap();
    assert_eq!(c_row.words, 3);
    assert_eq!(c_row.c_terms, 3);
    assert_eq!(c_row.restate, 2);
    assert!((c_row.c_coeff - (2.0 / 3.0)).abs() < 1e-4);
}
