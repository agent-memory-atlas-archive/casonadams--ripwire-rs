use ripwire_rs::cli::execute_hotspots_with;
use ripwire_rs::git::history::CommitInfo;
use ripwire_rs::{
    FileInfo, IngestResult, Interner, Lang, McpState, SymKind, Symbol, ToXmlString,
    compute_hotspots,
};
use std::collections::HashMap;
use std::io::Cursor;

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
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/core.rs".to_string()],
        },
        CommitInfo {
            id: "c2".to_string(),
            timestamp: 2000,
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/core.rs".to_string()],
        },
        CommitInfo {
            id: "c3".to_string(),
            timestamp: 3000,
            author: "dev@example.com".to_string(),
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
