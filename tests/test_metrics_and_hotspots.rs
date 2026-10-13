use ripwire_rs::ToXmlString;
use ripwire_rs::cli::execute_hotspots_with;
use ripwire_rs::domain::{FileInfo, IngestResult, Interner, Lang, SymKind, Symbol};
use ripwire_rs::git::history::CommitInfo;
use ripwire_rs::mcp::McpState;
use ripwire_rs::metrics::{
    NESTING_BAR, compute_complexity, compute_hotspots, compute_readability, posnett_score,
};
use std::collections::HashMap;
use std::io::Cursor;

#[test]
fn test_mccabe_and_cognitive_complexity() {
    let simple_code = r#"
fn simple_add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
    let simple_m = compute_complexity(simple_code, Lang::Rust);
    assert_eq!(simple_m.cx, 1);
    assert_eq!(simple_m.ccx, 0);
    assert_eq!(simple_m.max_nest, 0);
    assert_eq!(simple_m.humps, 0);
    assert_eq!(simple_m.deep, 0);
    assert_eq!(simple_m.ev, 1);

    let complex_code = r#"
fn complex_logic(x: i32, y: i32) -> i32 {
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
    let complex_m = compute_complexity(complex_code, Lang::Rust);
    assert!(complex_m.cx >= 5, "cx should be >= 5, got {}", complex_m.cx);
    assert!(
        complex_m.ccx >= 10,
        "ccx should be >= 10, got {}",
        complex_m.ccx
    );
    assert!(
        complex_m.max_nest >= NESTING_BAR,
        "max_nest should be >= 4, got {}",
        complex_m.max_nest
    );
    assert!(
        complex_m.humps >= 1,
        "humps should be >= 1, got {}",
        complex_m.humps
    );
    assert!(
        complex_m.deep >= 1,
        "deep lines should be >= 1, got {}",
        complex_m.deep
    );
}

#[test]
fn test_halstead_volume_and_posnett_readability() {
    let code = r#"
pub fn fibonacci(n: u32) -> u32 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}
"#;
    let r = compute_readability(code);
    assert!(r.token_count > 10);
    assert!(r.operator_count > 0);
    assert!(r.operand_count > 0);
    assert!(r.vocabulary_count > 5);
    assert!(r.volume > 0.0);
    assert!(r.entropy > 0.0);
    assert_eq!(r.lines, 8);

    // Posnett score should be in valid probability range [0, 1]
    assert!((0.0..=1.0).contains(&r.posnett));

    // Verify manual score calculation matches formula
    let expected = posnett_score(r.volume, r.lines as f64, r.entropy);
    assert!((r.posnett - expected).abs() < 1e-6);
}

#[test]
fn test_hotspots_computation_and_ranking() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();

    let p0 = interner.intern("src/high_churn_high_ccx.rs");
    let p1 = interner.intern("src/low_churn_high_ccx.rs");
    let p2 = interner.intern("src/high_churn_low_ccx.rs");
    let p3 = interner.intern("src/zero_churn.rs");

    ingest.files.push(FileInfo {
        id: 0,
        path: p0,
        lang: Lang::Rust,
        line_count: 100,
        byte_size: 2000,
    });
    ingest.files.push(FileInfo {
        id: 1,
        path: p1,
        lang: Lang::Rust,
        line_count: 100,
        byte_size: 2000,
    });
    ingest.files.push(FileInfo {
        id: 2,
        path: p2,
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    });
    ingest.files.push(FileInfo {
        id: 3,
        path: p3,
        lang: Lang::Rust,
        line_count: 20,
        byte_size: 500,
    });

    let mut s0 = Symbol::new(
        0,
        interner.intern("monster_fn"),
        interner.intern("monster_fn"),
        SymKind::Function,
        Lang::Rust,
        0,
        15,
    );
    s0.ccx = 40;

    let mut s1 = Symbol::new(
        1,
        interner.intern("clean_fn"),
        interner.intern("clean_fn"),
        SymKind::Function,
        Lang::Rust,
        1,
        20,
    );
    s1.ccx = 30;

    let mut s2 = Symbol::new(
        2,
        interner.intern("quick_helper"),
        interner.intern("quick_helper"),
        SymKind::Function,
        Lang::Rust,
        2,
        5,
    );
    s2.ccx = 2;

    ingest.symbols.push(s0);
    ingest.symbols.push(s1);
    ingest.symbols.push(s2);

    let mut file_churn = HashMap::new();
    file_churn.insert("src/high_churn_high_ccx.rs".to_string(), 25);
    file_churn.insert("src/low_churn_high_ccx.rs".to_string(), 2);
    file_churn.insert("src/high_churn_low_ccx.rs".to_string(), 50);
    file_churn.insert("src/zero_churn.rs".to_string(), 0);

    let report = compute_hotspots(&ingest, &interner, &file_churn);
    assert_eq!(report.total_files, 4);
    assert_eq!(report.ranked_count, 3);
    assert_eq!(report.unranked_no_churn, 1);

    // Ranked order:
    // 1. high_churn_high_ccx: churn=25, ccx=40 -> score = 1000
    // 2. high_churn_low_ccx:  churn=50, ccx=2  -> score = 100
    // 3. low_churn_high_ccx:  churn=2,  ccx=30 -> score = 60
    assert_eq!(report.files[0].file_path, "src/high_churn_high_ccx.rs");
    assert_eq!(report.files[0].score, 1000);
    assert_eq!(report.files[0].top_fn_name, "monster_fn");
    assert_eq!(report.files[0].top_fn_ccx, 40);
    assert_eq!(report.files[0].top_fn_line, 15);

    assert_eq!(report.files[1].file_path, "src/high_churn_low_ccx.rs");
    assert_eq!(report.files[1].score, 100);

    assert_eq!(report.files[2].file_path, "src/low_churn_high_ccx.rs");
    assert_eq!(report.files[2].score, 60);

    let xml = report.to_xml_string();
    assert!(xml.contains(
        "<hotspots schema=\"ripwire.hotspots/v1\" window=\"12mo@HEAD\" files=\"4\" ranked=\"3\""
    ));
    assert!(xml.contains("<f p=\"src/high_churn_high_ccx.rs\" churn=\"25\" ccx=\"40\" score=\"1000\" top=\"monster_fn\" top_ccx=\"40\" top_l=\"15\"/>"));
}

#[test]
fn test_cli_hotspots_execution_xml_output() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();

    let p = interner.intern("src/core.rs");
    ingest.files.push(FileInfo {
        id: 0,
        path: p,
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    });

    let mut sym = Symbol::new(
        0,
        interner.intern("process_data"),
        interner.intern("process_data"),
        SymKind::Function,
        Lang::Rust,
        0,
        12,
    );
    sym.ccx = 15;
    ingest.symbols.push(sym);

    let state = McpState::from_ingest(".", ingest, interner);
    let commits = vec![
        CommitInfo {
            id: "c1".to_string(),
            timestamp: 1000,
            changed_files: vec!["src/core.rs".to_string()],
        },
        CommitInfo {
            id: "c2".to_string(),
            timestamp: 2000,
            changed_files: vec!["src/core.rs".to_string()],
        },
        CommitInfo {
            id: "c3".to_string(),
            timestamp: 3000,
            changed_files: vec!["src/core.rs".to_string()],
        },
    ];

    let mut out = Cursor::new(Vec::new());
    execute_hotspots_with(None, &commits, &state, &mut out).expect("execute hotspots succeeds");

    let xml = String::from_utf8(out.into_inner()).unwrap();
    assert!(xml.contains(
        "<hotspots schema=\"ripwire.hotspots/v1\" window=\"12mo@HEAD\" files=\"1\" ranked=\"1\""
    ));
    assert!(xml.contains("<f p=\"src/core.rs\" churn=\"3\" ccx=\"15\" score=\"45\" top=\"process_data\" top_ccx=\"15\" top_l=\"12\"/>"));
}
