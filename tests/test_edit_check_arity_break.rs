use rwire::cli::{execute_affected_with, execute_edit_check_with};
use rwire::domain::{FileInfo, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol};
use rwire::gate::affected::find_affected_tests;
use rwire::gate::contract::SignatureContract;
use rwire::gate::edit_check::check_symbol_contract;
use rwire::graph::Graph;
use rwire::mcp::{dispatch_json_rpc, McpState};
use rwire::ToXmlString;
use serde_json::Value;
use std::io::Cursor;

fn setup_test_repository(interner: &Interner) -> IngestResult {
    let mut ingest = IngestResult::new();

    // File 0: src/service.rs
    let p_service = interner.intern("src/service.rs");
    ingest.files.push(FileInfo {
        id: 0,
        path: p_service,
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    });

    // File 1: src/legacy_caller.rs
    let p_legacy = interner.intern("src/legacy_caller.rs");
    ingest.files.push(FileInfo {
        id: 1,
        path: p_legacy,
        lang: Lang::Rust,
        line_count: 40,
        byte_size: 800,
    });

    // File 2: src/modern_caller.rs
    let p_modern = interner.intern("src/modern_caller.rs");
    ingest.files.push(FileInfo {
        id: 2,
        path: p_modern,
        lang: Lang::Rust,
        line_count: 40,
        byte_size: 800,
    });

    // File 3: tests/test_service.rs
    let p_test = interner.intern("tests/test_service.rs");
    ingest.files.push(FileInfo {
        id: 3,
        path: p_test,
        lang: Lang::Rust,
        line_count: 60,
        byte_size: 1200,
    });

    // Symbol 0: target function `compute` (now 3 parameters in working tree)
    let s_compute = interner.intern("compute");
    let mut sym_compute = Symbol::new(
        0,
        s_compute,
        s_compute,
        SymKind::Function,
        Lang::Rust,
        0,
        10,
    );
    sym_compute.params = 3;

    // Symbol 1: `legacy_worker` in src/legacy_caller.rs (calls compute with 2 args)
    let s_legacy = interner.intern("legacy_worker");
    let sym_legacy = Symbol::new(1, s_legacy, s_legacy, SymKind::Function, Lang::Rust, 1, 15);

    // Symbol 2: `modern_worker` in src/modern_caller.rs (calls compute with 3 args)
    let s_modern = interner.intern("modern_worker");
    let sym_modern = Symbol::new(2, s_modern, s_modern, SymKind::Function, Lang::Rust, 2, 20);

    // Symbol 3: `test_integration` in tests/test_service.rs (calls legacy_worker)
    let s_test = interner.intern("test_integration");
    let sym_test = Symbol::new(3, s_test, s_test, SymKind::Function, Lang::Rust, 3, 25);

    ingest.symbols.push(sym_compute);
    ingest.symbols.push(sym_legacy);
    ingest.symbols.push(sym_modern);
    ingest.symbols.push(sym_test);

    // Call 1: legacy_worker -> compute(a, b) [2 arguments]
    let mut r1 = Reference::new(1, s_compute, 1, 18, Lang::Rust, RefRole::Call);
    r1.arg_count = 2;
    r1.arg_count_known = true;

    // Call 2: modern_worker -> compute(a, b, c) [3 arguments]
    let mut r2 = Reference::new(2, s_compute, 2, 22, Lang::Rust, RefRole::Call);
    r2.arg_count = 3;
    r2.arg_count_known = true;

    // Call 3: test_integration -> legacy_worker()
    let mut r3 = Reference::new(3, s_legacy, 3, 28, Lang::Rust, RefRole::Call);
    r3.arg_count = 0;
    r3.arg_count_known = true;

    ingest.references.push(r1);
    ingest.references.push(r2);
    ingest.references.push(r3);

    ingest
}

#[test]
fn test_edit_check_arity_break() {
    let interner = Interner::new();
    let ingest = setup_test_repository(&interner);
    let graph = Graph::build(&ingest, &interner);

    // Baseline from git HEAD: compute had 2 parameters
    let baseline = SignatureContract {
        name: "compute".to_string(),
        params: 2,
        is_public: true,
    };

    let report = check_symbol_contract(0, Some(&baseline), &ingest, &graph, &interner)
        .expect("check_symbol_contract report");

    assert_eq!(report.symbol_name, "compute");
    assert_eq!(report.total_callers, 2);
    assert_eq!(report.incompatible_callers, 1);

    // Verify broken caller is legacy_worker
    let legacy_caller = report
        .callers
        .iter()
        .find(|c| c.caller_name == "legacy_worker")
        .expect("legacy_worker caller");
    assert!(legacy_caller.is_incompatible);
    assert_eq!(legacy_caller.sites.len(), 1);
    assert_eq!(legacy_caller.sites[0].arg_count, 2);
    assert_eq!(legacy_caller.sites[0].required_params, 3);
    assert_eq!(legacy_caller.sites[0].line, 18);

    // Verify modern_worker is compatible
    let modern_caller = report
        .callers
        .iter()
        .find(|c| c.caller_name == "modern_worker")
        .expect("modern_worker caller");
    assert!(!modern_caller.is_incompatible);
    assert!(modern_caller.sites.is_empty());

    // Verify XML formatting
    let xml = report.to_xml_string();
    assert!(xml.contains("<edit-check sym=\"compute\" p=\"src/service.rs\" status=\"contract-change\" callers=\"2\" incompatible=\"1\">"));
    assert!(xml.contains(
        "<caller sym=\"legacy_worker\" p=\"src/legacy_caller.rs:15\" incompatible=\"1\">"
    ));
    assert!(xml.contains("<site line=\"18\" args=\"2\" required=\"3\"/>"));
    assert!(xml.contains("<caller sym=\"modern_worker\" p=\"src/modern_caller.rs:20\"/>"));
}

#[test]
fn test_affected_tests_transitive_reachability() {
    let interner = Interner::new();
    let ingest = setup_test_repository(&interner);
    let graph = Graph::build(&ingest, &interner);

    // Query affected tests for changed file "src/service.rs"
    let report = find_affected_tests(&["src/service.rs"], &ingest, &graph, &interner);
    assert_eq!(report.seed_count, 1);
    assert_eq!(report.total_tests, 1);

    let test = &report.tests[0];
    assert_eq!(test.path, "tests/test_service.rs");
    // Path: compute <- legacy_worker <- test_integration = 2 hops
    assert_eq!(test.hops, 2);
    assert!(!test.is_direct_change);

    assert_eq!(report.reached_symbols, 4);
    let xml = report.to_xml_string();
    assert!(xml.contains("<affected changed=\"src/service.rs\" seeds=\"1\" seed_test_files=\"0\" tests=\"1\" reached=\"4\">"));
    assert!(xml.contains("<test path=\"tests/test_service.rs\" hops=\"2\"/>"));
}

#[test]
fn test_cli_gate_execution_and_xml_output() {
    let interner = Interner::new();
    let ingest = setup_test_repository(&interner);
    let state = McpState::from_ingest(".", ingest, interner);

    // 1. Test CLI edit-check
    let mut out_edit = Cursor::new(Vec::new());
    execute_edit_check_with("compute", &state, &mut out_edit).expect("execute edit-check");
    let xml_edit = String::from_utf8(out_edit.into_inner()).unwrap();
    assert!(xml_edit.contains("<edit-check sym=\"compute\""));

    // 2. Test CLI affected
    let mut out_aff = Cursor::new(Vec::new());
    execute_affected_with("src/service.rs", &state, &mut out_aff).expect("execute affected");
    let xml_aff = String::from_utf8(out_aff.into_inner()).unwrap();
    assert!(xml_aff.contains("<affected changed=\"src/service.rs\""));
    assert!(xml_aff.contains("<test path=\"tests/test_service.rs\""));
}

#[test]
fn test_mcp_edit_check_and_affected_tools() {
    let interner = Interner::new();
    let ingest = setup_test_repository(&interner);
    let mut state = Some(McpState::from_ingest(".", ingest, interner));

    // 1. MCP edit_check tool call
    let req_edit = r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"edit_check","arguments":{"symbol":"compute"}}}"#;
    let resp_edit = dispatch_json_rpc(req_edit, &mut state, ".").expect("reply to edit_check");
    let val_edit: Value = serde_json::from_str(&resp_edit).unwrap();
    assert_eq!(val_edit["id"], 10);
    let text_edit = val_edit["result"]["content"][0]["text"].as_str().unwrap();
    let data_edit: Value = serde_json::from_str(text_edit).unwrap();
    assert_eq!(data_edit["symbol"], "compute");
    assert_eq!(data_edit["total_callers"], 2);

    // 2. MCP affected tool call
    let req_aff = r#"{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"affected","arguments":{"files":"src/service.rs"}}}"#;
    let resp_aff = dispatch_json_rpc(req_aff, &mut state, ".").expect("reply to affected");
    let val_aff: Value = serde_json::from_str(&resp_aff).unwrap();
    assert_eq!(val_aff["id"], 11);
    let text_aff = val_aff["result"]["content"][0]["text"].as_str().unwrap();
    let data_aff: Value = serde_json::from_str(text_aff).unwrap();
    assert_eq!(data_aff["changed"], "src/service.rs");
    assert_eq!(data_aff["total_tests"], 1);
    assert_eq!(data_aff["tests"][0]["path"], "tests/test_service.rs");
}
