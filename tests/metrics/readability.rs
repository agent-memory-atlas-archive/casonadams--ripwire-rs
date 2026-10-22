use ripwire_rs::metrics::execute_biggest_first_query;
use ripwire_rs::serialize::PageRequest;
use ripwire_rs::{
    FileInfo, IngestResult, Interner, Lang, SymKind, Symbol, compute_readability, posnett_score,
};

#[test]
fn test_posnett_score_calculation() {
    let p = posnett_score(100.0, 10.0, 3.5);
    assert!((0.0..=1.0).contains(&p));
}

#[test]
fn test_readability_metrics_computation() {
    let code = r#"
pub fn fibonacci(n: u32) -> u32 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}
"#;
    let r = compute_readability(code, Lang::Rust);
    assert!(r.token_count > 10);
    assert!(r.operator_count > 0);
    assert!(r.operand_count > 0);
    assert!(r.vocabulary_count > 5);
    assert!(r.volume > 0.0);
    assert!(r.entropy > 0.0);
    assert_eq!(r.lines, 8);
    assert!((0.0..=1.0).contains(&r.posnett));

    let expected = posnett_score(r.volume, r.lines as f64, r.entropy);
    assert!((r.posnett - expected).abs() < 1e-6);
}

#[test]
fn test_biggest_first_without_source_reports_no_fabricated_halstead() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern("src/long.rs"),
        lang: Lang::Rust,
        line_count: 10,
        byte_size: 100,
    });
    let name = interner.intern("long_fn");
    let mut sym = Symbol::new(0, name, name, SymKind::Function, Lang::Rust, 0, 10);
    sym.loc = 10;
    ingest.symbols.push(sym);
    let mut buf = Vec::new();
    execute_biggest_first_query(
        "/nonexistent/ripwire-readability-root",
        &ingest,
        &interner,
        PageRequest::default(),
        &mut buf,
    )
    .unwrap();
    let xml = String::from_utf8(buf).unwrap();
    assert!(xml.contains("toks=\"0\""), "{xml}");
    assert!(xml.contains("ent=\"0.00\""), "{xml}");
    assert!(!xml.contains("ent=\"4.20\""), "{xml}");
}
