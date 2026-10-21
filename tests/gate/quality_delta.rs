use ripwire_rs::cli::{GateDispatchArgs, execute_quality_delta_with_args};
use ripwire_rs::gate::baseline::{
    QualitySnapshot, compute_snapshot, parse_snapshot_from_str, serialize_snapshot_to_string,
};
use ripwire_rs::gate::quality_delta::{
    QualityRegression, StaleAck, ack_matching_regressions, body_text_kinds,
    collect_changed_clone_regressions, compute_quality_delta, compute_quality_delta_with_acks,
    content_id, is_changed_callable, parse_acks_content, split_ack_patterns, stale_replay_for,
    symbol_matches_and_changed,
};
use ripwire_rs::metrics::source_mask::mask_source;
use ripwire_rs::{
    CloneGroup, CloneIdiom, CloneType, FileInfo, Graph, IngestResult, Interner, Lang, McpState,
    SymKind, Symbol, ToXmlString, dispatch_json_rpc,
};
use serde_json::Value;
use std::io::Cursor;

fn setup_baseline_repository(interner: &Interner) -> (IngestResult, Graph) {
    let mut ingest = IngestResult::new();

    let p = interner.intern("src/calc.rs");
    ingest.files.push(FileInfo {
        id: 0,
        path: p,
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    });

    let s_name = interner.intern("evaluate");
    let mut sym = Symbol::new(0, s_name, s_name, SymKind::Function, Lang::Rust, 0, 10);
    sym.ccx = 10;
    sym.loc = 25;
    sym.max_nest = 2;
    sym.params = 2;

    ingest.symbols.push(sym);
    let graph = Graph::build(&ingest, interner);
    (ingest, graph)
}

#[test]
fn test_snapshot_computation_and_serialization() {
    let interner = Interner::new();
    let (ingest, graph) = setup_baseline_repository(&interner);

    let snap = compute_snapshot(&ingest, &graph, &interner, "test_sha_123");
    assert_eq!(snap.head_sha, "test_sha_123");
    assert_eq!(snap.ccx_by_sym["src/calc.rs::evaluate"], 10);
    assert_eq!(snap.loc_by_sym["src/calc.rs::evaluate"], 25);
    assert_eq!(snap.nest_by_sym["src/calc.rs::evaluate"], 2);
    assert_eq!(snap.params_by_sym["src/calc.rs::evaluate"], 2);

    let serialized = serialize_snapshot_to_string(&snap);
    assert!(serialized.contains("head test_sha_123"));
    assert!(serialized.contains("ccx src/calc.rs::evaluate 10"));

    let deserialized = parse_snapshot_from_str(&serialized);
    assert_eq!(deserialized.head_sha, snap.head_sha);
    assert_eq!(deserialized.ccx_by_sym, snap.ccx_by_sym);
}

#[test]
fn test_quality_delta_clean_vs_regression() {
    let mut baseline = QualitySnapshot {
        head_sha: "sha_base".to_string(),
        ..Default::default()
    };
    baseline
        .ccx_by_sym
        .insert("src/calc.rs::evaluate".to_string(), 10);
    baseline
        .loc_by_sym
        .insert("src/calc.rs::evaluate".to_string(), 25);

    let mut clean_working = baseline.clone();
    let clean_report = compute_quality_delta(&clean_working, &baseline, "test-base");
    assert_eq!(clean_report.regressions.len(), 0);
    assert_eq!(clean_report.gating_count, 0);
    assert!(!clean_report.has_gating_failures());

    clean_working
        .ccx_by_sym
        .insert("src/calc.rs::evaluate".to_string(), 25);
    clean_working
        .line_by_sym
        .insert("src/calc.rs::evaluate".to_string(), 7);
    let reg_report = compute_quality_delta(&clean_working, &baseline, "test-base");
    assert_eq!(reg_report.regressions.len(), 1);
    assert_eq!(reg_report.gating_count, 1);
    assert!(reg_report.has_gating_failures());

    let reg = &reg_report.regressions[0];
    assert_eq!(reg.kind, "complexity");
    assert_eq!(reg.sym_name, "evaluate");
    assert_eq!(reg.was_val, 10);
    assert_eq!(reg.now_val, 25);
    assert!(reg.is_gating);
    assert!(!reg.is_new_symbol);

    let xml = reg_report.to_xml_string();
    assert!(xml.contains("gating=\"1\""));
    assert!(xml.contains("<r kind=\"complexity\" sym=\"evaluate\" p=\"src/calc.rs:7\" was=\"10\" now=\"25\" gating=\"1\" bar=\"15\"/>"));
}

#[test]
fn test_quality_delta_new_symbol_and_dead_code() {
    let baseline = QualitySnapshot::default();
    let mut working = QualitySnapshot::default();

    working
        .ccx_by_sym
        .insert("src/new.rs::monster".to_string(), 35);
    working
        .dead_symbols
        .insert("src/new.rs::monster".to_string());

    let report = compute_quality_delta(&working, &baseline, "head");
    assert_eq!(report.new_symbol_count, 2);
    assert_eq!(report.gating_count, 0);
    assert!(!report.has_gating_failures());

    let xml = report.to_xml_string();
    assert!(xml.contains("origin=\"new-symbol\""));
}

#[test]
fn test_cli_quality_delta_xml_execution() {
    let interner = Interner::new();
    let (ingest, _graph) = setup_baseline_repository(&interner);
    let state = McpState::from_ingest(".", ingest, interner);

    let mut out = Cursor::new(Vec::new());
    let has_failures =
        execute_quality_delta_with_args(GateDispatchArgs::default(), &state, &mut out)
            .expect("execute quality-delta");
    assert!(!has_failures);

    let xml = String::from_utf8(out.into_inner()).unwrap();
    assert!(xml.contains("<quality-delta"));
}

#[test]
fn test_mcp_quality_delta_json_rpc() {
    let interner = Interner::new();
    let (ingest, _graph) = setup_baseline_repository(&interner);
    let mut state = Some(McpState::from_ingest(".", ingest, interner));

    let req =
        r#"{"jsonrpc":"2.0","id":100,"method":"tools/call","params":{"name":"quality_delta"}}"#;
    let resp = dispatch_json_rpc(req, &mut state, ".").expect("reply to quality_delta");
    let val: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(val["id"], 100);

    let text = val["result"]["content"][0]["text"].as_str().unwrap();
    let data: Value = serde_json::from_str(text).unwrap();
    assert_eq!(data["gating_count"], 0);
    assert_eq!(data["has_gating_failures"], false);
}

#[test]
fn test_changed_callable_and_clone_regressions_in_memory() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern("src/a.rs"),
        lang: Lang::Rust,
        line_count: 20,
        byte_size: 200,
    });
    let sym = Symbol::new(
        0,
        interner.intern("dup_fn"),
        interner.intern("src/a.rs::dup_fn"),
        SymKind::Function,
        Lang::Rust,
        0,
        5,
    );
    ingest.symbols.push(sym);
    let changed = vec!["src/a.rs".to_string()];
    assert!(is_changed_callable(&sym, "src/a.rs", &changed));
    assert!(!is_changed_callable(&sym, "src/b.rs", &changed));
    let state = McpState::from_ingest(":memory:", ingest, interner);
    let cg = CloneGroup {
        members: vec![0],
        clone_type: CloneType::Type1,
        token_count: 80,
        similarity: 1.0,
        idiom: CloneIdiom::None,
        tokens_saved: 80,
    };
    let mut regs = Vec::new();
    collect_changed_clone_regressions(vec![cg], &state, &changed, &mut regs);
    assert_eq!(regs.len(), 1);
    assert_eq!(regs[0].kind, "duplication");
    assert_eq!(regs[0].line, 5);
}

#[test]
fn test_body_text_kinds_ignore_literals_and_read_comments_for_markers() {
    let kinds = |src: &str, lang: Lang| body_text_kinds(&mask_source(src, lang));

    let quoted = "fn f() { let s = \"TODO: catch {} todo!( std::unique(\"; }";
    assert!(kinds(quoted, Lang::Rust).is_empty());
    assert_eq!(
        kinds("fn f() { // TODO: wire it\n}", Lang::Rust),
        ["placeholder"]
    );
    assert_eq!(kinds("fn f() { todo!() }", Lang::Rust), ["placeholder"]);
    assert_eq!(
        kinds(
            "def f():\n    try:\n        g()\n    except:\n        pass\n",
            Lang::Python
        ),
        ["error-masking"]
    );
    assert_eq!(
        kinds("void f() { try { g(); } catch (...) { } }", Lang::Cpp),
        ["error-masking"]
    );
    assert_eq!(
        kinds("void f() { std::unique(v.begin(), v.end()); }", Lang::Cpp),
        ["defect-shape"]
    );
    assert!(
        kinds(
            "void f() { std::sort(a, b); std::unique(a, b); }",
            Lang::Cpp
        )
        .is_empty()
    );
}

#[test]
fn test_stale_rows_collapse_unless_show_stale() {
    let snap = QualitySnapshot::default();
    let mut report = compute_quality_delta_with_acks(&snap, &snap, "git-HEAD", None);
    let stale = |kind: &str, key: &str| StaleAck {
        kind: kind.to_string(),
        key: key.to_string(),
        why: "finding-gone",
        sym: key.to_string(),
        path: "src/a.rs:1".to_string(),
    };
    report.stale_acks = vec![
        stale("complexity", "k1"),
        stale("dead-code:new-symbol", "k2"),
        stale("complexity", "k3"),
    ];

    report.stale_collapse_replay = Some("--quality-delta --scope=src".to_string());
    let collapsed = report.to_xml_string();
    assert!(collapsed.contains("stale=\"3\""));
    assert!(collapsed.contains("stale_by_kind=\"complexity:2,dead-code:1\""));
    assert!(collapsed.contains("stale_next=\"--quality-delta --scope=src --show-stale\""));
    assert!(!collapsed.contains("<sa "));

    report.stale_collapse_replay = None;
    let full = report.to_xml_string();
    assert_eq!(full.matches("<sa ").count(), 3);
    assert!(!full.contains(" stale_by_kind=\""));
}

#[test]
fn test_ack_only_filter_marks_and_records_matching_rows() {
    let snap = QualitySnapshot::default();
    let mut report = compute_quality_delta_with_acks(&snap, &snap, "git-HEAD", None);
    let row = |kind: &str, sym: &str| QualityRegression {
        kind: kind.to_string(),
        sym_name: sym.to_string(),
        file_path: "src/a.rs".to_string(),
        line: 3,
        was_val: 10,
        now_val: 20,
        is_gating: true,
        is_new_symbol: false,
    };
    report.regressions = vec![
        row("complexity", "calc"),
        row("nesting", "walk"),
        row("duplication", "calc"),
    ];
    let mut ledger = Vec::new();
    let cid_for = |r: &QualityRegression| (r.kind != "duplication").then_some(0xabcu64);
    ack_matching_regressions(
        &mut report,
        &split_ack_patterns(" complexity ,duplication"),
        "why",
        cid_for,
        &mut ledger,
    )
    .unwrap();
    let text = String::from_utf8(ledger).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("ack complexity calc 20 cid=0000000000000abc now=20"));
    assert!(lines[0].ends_with("p=src/a.rs:3 why"));
    assert!(lines[1].starts_with("ack duplication calc 20 now=20"));
    assert!(!text.contains("cid=0000000000000000"));
    assert_eq!(report.acked_count, 2);
    assert!(!report.regressions[0].is_gating);
    assert!(report.regressions[1].is_gating);
    assert_eq!(stale_replay_for(Some("src")), "--quality-delta --scope=src");
}

#[test]
fn test_content_id_survives_reindent_and_self_rename_only() {
    let base = content_id("fn calc(x: i32) -> i32 {\n    x + 1\n}", "calc");
    let reindented = content_id("fn calc(x: i32) -> i32 {\n\t\tx + 1\n}", "calc");
    let renamed = content_id("fn total(x: i32) -> i32 {\n    x + 1\n}", "total");
    let changed = content_id("fn calc(x: i32) -> i32 {\n    x + 2\n}", "calc");
    assert_eq!(base, reindented);
    assert_eq!(base, renamed);
    assert_ne!(base, changed);
    assert_ne!(base, 0);
}

#[test]
fn test_quality_delta_nesting_and_params_regressions() {
    let mut baseline = QualitySnapshot::default();
    baseline
        .ccx_by_sym
        .insert("src/math.rs::deep".to_string(), 5);
    baseline
        .nest_by_sym
        .insert("src/math.rs::deep".to_string(), 2);
    baseline
        .params_by_sym
        .insert("src/math.rs::deep".to_string(), 2);

    let mut working = QualitySnapshot::default();
    working
        .ccx_by_sym
        .insert("src/math.rs::deep".to_string(), 5);
    working
        .nest_by_sym
        .insert("src/math.rs::deep".to_string(), 7);
    working
        .params_by_sym
        .insert("src/math.rs::deep".to_string(), 6);

    let report = compute_quality_delta(&working, &baseline, "test-baseline");
    assert_eq!(report.regressions.len(), 2);
    assert_eq!(report.gating_count, 2);
    assert!(report.has_gating_failures());

    let kinds: Vec<&str> = report.regressions.iter().map(|r| r.kind.as_str()).collect();
    assert!(kinds.contains(&"nesting"));
    assert!(kinds.contains(&"params"));
}

#[test]
fn test_quality_delta_acks_suppression() {
    let mut baseline = QualitySnapshot::default();
    baseline
        .ccx_by_sym
        .insert("src/math.rs::deep".to_string(), 5);
    baseline
        .nest_by_sym
        .insert("src/math.rs::deep".to_string(), 2);

    let mut working = QualitySnapshot::default();
    working
        .ccx_by_sym
        .insert("src/math.rs::deep".to_string(), 5);
    working
        .nest_by_sym
        .insert("src/math.rs::deep".to_string(), 7);

    let acks_content = "ack nesting 70a00a1f5e885d55 10 p=src/math.rs:1 deliberate deep nesting\n";
    let acks = parse_acks_content(acks_content);

    let report = compute_quality_delta_with_acks(&working, &baseline, "test-baseline", Some(&acks));
    assert_eq!(report.regressions.len(), 1);
    assert_eq!(report.gating_count, 0);
    assert!(!report.has_gating_failures());
}

#[test]
fn test_symbol_matches_and_changed() {
    let mut baseline = QualitySnapshot::default();
    baseline
        .ccx_by_sym
        .insert("src/math.rs::calc".to_string(), 10);
    baseline
        .loc_by_sym
        .insert("src/math.rs::calc".to_string(), 20);

    let interner = Interner::new();
    let sym_name = interner.intern("calc");
    let path = interner.intern("src/math.rs");
    let mut sym = Symbol::new(
        0,
        sym_name,
        interner.intern("src/math.rs::calc"),
        SymKind::Function,
        Lang::Rust,
        0,
        10,
    );
    sym.loc = 30;
    sym.ccx = 10;
    let ingest = IngestResult {
        files: vec![FileInfo {
            id: 0,
            path,
            lang: Lang::Rust,
            byte_size: 100,
            line_count: 100,
        }],
        symbols: vec![sym],
        references: Vec::new(),
        ..IngestResult::default()
    };
    let changed = symbol_matches_and_changed(&sym, "src/math.rs", &ingest, &interner, &baseline);
    assert!(changed.is_some());
}
