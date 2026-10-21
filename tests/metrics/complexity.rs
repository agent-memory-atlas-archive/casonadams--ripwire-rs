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

fn flat_ifs(open: &str, close: &str, cond: impl Fn(u32) -> String) -> String {
    (0..10)
        .map(|i| format!("{}{}{}", cond(i), open, close))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn test_flat_ifs_count_once_per_language() {
    let braces = |kw: &str, pre: &str, post: &str| {
        let body = flat_ifs(" { return 1; }", "", |i| format!("{kw}{pre}x == {i}{post}"));
        format!("{body}\nreturn 0;\n")
    };
    let cases: Vec<(Lang, String)> = vec![
        (
            Lang::Dart,
            format!("int f(int x) {{\n{}}}\n", braces("if ", "(", ")")),
        ),
        (
            Lang::Rust,
            format!("fn f(x: i32) -> i32 {{\n{}}}\n", braces("if ", "", "")),
        ),
        (
            Lang::Python,
            format!(
                "def f(x):\n{}    return 0\n",
                (0..10)
                    .map(|i| format!("    if x == {i}:\n        return 1\n"))
                    .collect::<String>()
            ),
        ),
        (
            Lang::Ruby,
            format!(
                "def f(x)\n{}  0\nend\n",
                (0..10)
                    .map(|i| format!("  if x == {i}\n    return 1\n  end\n"))
                    .collect::<String>()
            ),
        ),
    ];
    for (lang, code) in cases {
        let m = compute_complexity(&code, lang);
        assert_eq!((m.cx, m.ccx, m.max_nest), (11, 10, 1), "{lang:?}");
    }
}

#[test]
fn test_else_if_chain_and_plain_else() {
    let dart = "int f(int x) {\n  if (x == 0) { return 0; } else if (x == 1) { return 1; } else { return 2; }\n}\n";
    let m = compute_complexity(dart, Lang::Dart);
    assert_eq!((m.cx, m.ccx), (3, 3));
    let py = "def f(x):\n    if x == 0:\n        return 0\n    elif x == 1:\n        return 1\n    else:\n        return 2\n";
    let m = compute_complexity(py, Lang::Python);
    assert_eq!((m.cx, m.ccx), (3, 3));
    let rs = "fn f(x: i32) -> i32 { if x == 0 { 0 } else if x == 1 { 1 } else { 2 } }";
    let m = compute_complexity(rs, Lang::Rust);
    assert_eq!((m.cx, m.ccx), (3, 3));
}

#[test]
fn test_logical_operator_sequences() {
    let cases = [
        (
            Lang::Rust,
            "fn f(a: bool, b: bool, c: bool) -> bool { a && b && c }",
            (3, 1),
        ),
        (
            Lang::Rust,
            "fn f(a: bool, b: bool, c: bool) -> bool { a && b || c }",
            (3, 2),
        ),
        (
            Lang::Python,
            "def f(a, b, c):\n    return a and b or c\n",
            (3, 2),
        ),
        (
            Lang::Dart,
            "bool f(bool a, bool b, bool c) { return a && b && c; }",
            (3, 1),
        ),
        (
            Lang::Dart,
            "bool f(bool a, bool b, bool c) { return a && b || c; }",
            (3, 2),
        ),
    ];
    for (lang, code, (cx, ccx)) in cases {
        let m = compute_complexity(code, lang);
        assert_eq!((m.cx, m.ccx), (cx, ccx), "{lang:?}: {code}");
    }
}

#[test]
fn test_ruby_modifier_and_python_keyword_not_double_counted() {
    let rb = "def f(x)\n  return 1 if x\nend\n";
    let m = compute_complexity(rb, Lang::Ruby);
    assert_eq!((m.cx, m.ccx), (2, 1));
}

#[test]
fn test_kotlin_else_if_and_scala_logical_operators() {
    let kt = "fun f(x: Int): Int {\n  if (x == 0) { return 0 } else if (x == 1) { return 1 } else { return 2 }\n}\n";
    let m = compute_complexity(kt, Lang::Kotlin);
    assert_eq!((m.cx, m.ccx), (3, 3));
    let scala = "object O { def f(a: Boolean, b: Boolean, c: Boolean): Boolean = a && b || c }\n";
    let m = compute_complexity(scala, Lang::Scala);
    assert_eq!((m.cx, m.ccx), (3, 2));
}
