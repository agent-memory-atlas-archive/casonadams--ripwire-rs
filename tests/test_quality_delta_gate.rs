use ripwire_rs::ToXmlString;
use ripwire_rs::cli::execute_quality_delta_with;
use ripwire_rs::domain::{FileInfo, IngestResult, Interner, Lang, SymKind, Symbol};
use ripwire_rs::gate::baseline::{
    QualitySnapshot, compute_snapshot, parse_snapshot_from_str, serialize_snapshot_to_string,
};
use ripwire_rs::gate::quality_delta::compute_quality_delta;
use ripwire_rs::graph::Graph;
use ripwire_rs::mcp::{McpState, dispatch_json_rpc};
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

    // 1. Clean working tree: no regression
    let mut clean_working = baseline.clone();
    let clean_report = compute_quality_delta(&clean_working, &baseline, "test-base");
    assert_eq!(clean_report.regressions.len(), 0);
    assert_eq!(clean_report.gating_count, 0);
    assert!(!clean_report.has_gating_failures());

    // 2. Gating regression: ccx grows to 25 (> 15 bar)
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

    // New symbol with high complexity
    working
        .ccx_by_sym
        .insert("src/new.rs::monster".to_string(), 35);
    // Newly introduced dead code
    working
        .dead_symbols
        .insert("src/new.rs::monster".to_string());

    let report = compute_quality_delta(&working, &baseline, "head");
    assert_eq!(report.new_symbol_count, 2);
    // New symbols are tracked debt, but do not gate pre-existing failure
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
    let has_failures = execute_quality_delta_with(&state, &mut out).expect("execute quality-delta");
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
