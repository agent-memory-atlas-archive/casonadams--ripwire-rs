use ripwire_rs::{Lang, compute_readability, posnett_score};

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
