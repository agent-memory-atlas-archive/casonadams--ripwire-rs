use ripwire_rs::{Lang, NESTING_BAR, compute_complexity};

#[test]
fn test_estimate_text_complexity_fallback() {
    let code = "if true { println!(); }";
    let m = compute_complexity(code, Lang::Unknown);
    assert!(m.cx >= 2);
    assert!(m.ccx >= 1);
}

#[test]
fn test_complexity_linear_function() {
    let code = "fn add(a: i32, b: i32) -> i32 { a + b }";
    let m = compute_complexity(code, Lang::Rust);
    assert_eq!(m.cx, 1);
    assert_eq!(m.ccx, 0);
    assert_eq!(m.max_nest, 0);
    assert_eq!(m.humps, 0);
    assert_eq!(m.deep, 0);
    assert_eq!(m.ev, 1);
}

#[test]
fn test_complexity_nested_if_and_humps() {
    let code = r#"
fn test_func(x: i32, y: i32) -> i32 {
    if x > 0 {
        for i in 0..x {
            if i % 2 == 0 {
                while y > i {
                    if x == y {
                        println!("deep nesting");
                    }
                }
            }
        }
    }
    x + y
}
"#;
    let m = compute_complexity(code, Lang::Rust);
    assert!(m.cx >= 5);
    assert!(m.ccx >= 10);
    assert!(m.max_nest >= NESTING_BAR);
    assert!(m.humps >= 1);
    assert!(m.deep >= 1);
}
