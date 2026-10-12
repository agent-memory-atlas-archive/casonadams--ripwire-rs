use ripwire::cli::{
    GateArgs, MetricsArgs, NavMode, NavigateArgs, SliceArgs, execute_nav_symbol_query,
    execute_navigate_with, execute_stream_xml_with,
};
use ripwire::{
    Cli, Commands, Graph, Interner, Lang, LanguageRegistry, McpState, PageRankConfig,
    dispatch_json_rpc, ingest_in_memory_files, run_cli_args, run_mcp_stream,
};
use serde_json::Value;
use std::io::Cursor;

#[test]
fn test_cli_help_categories() {
    use clap::CommandFactory;

    let mut cmd = Cli::command();
    let help_str = cmd.render_help().to_string();

    // Verify all 6 categorized workflow groups are present
    assert!(help_str.contains("orient"), "must contain 'orient'");
    assert!(help_str.contains("navigate"), "must contain 'navigate'");
    assert!(help_str.contains("gate"), "must contain 'gate'");
    assert!(help_str.contains("metrics"), "must contain 'metrics'");
    assert!(help_str.contains("slice"), "must contain 'slice'");
    assert!(help_str.contains("mcp"), "must contain 'mcp'");
}

#[test]
fn test_mcp_json_rpc_suite() {
    // 100% in-memory fixture: zero disk I/O, zero network
    let fixtures: &[(&str, Lang, &[u8])] = &[(
        "lib.rs",
        Lang::Rust,
        b"pub fn run_cli() { helper(); } fn helper() {}",
    )];
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(fixtures, &reg, &interner);
    let mut state = Some(McpState::from_ingest(".", ingest, interner));

    // ── 1. INITIALIZE ────────────────────────────────────────────────────────
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#;
    let init_resp = dispatch_json_rpc(init_req, &mut state, ".").expect("must reply to initialize");
    let init_val: Value = serde_json::from_str(&init_resp).unwrap();
    assert_eq!(init_val["id"], 1);
    assert_eq!(init_val["result"]["serverInfo"]["name"], "ripwire");
    assert_eq!(init_val["result"]["protocolVersion"], "2024-11-05");

    // ── 2. NOTIFICATION: notifications/initialized (no reply) ────────────────
    let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    assert!(dispatch_json_rpc(notif, &mut state, ".").is_none());

    // ── 3. TOOLS/LIST ────────────────────────────────────────────────────────
    let list_req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
    let list_resp = dispatch_json_rpc(list_req, &mut state, ".").expect("must reply to tools/list");
    let list_val: Value = serde_json::from_str(&list_resp).unwrap();
    assert_eq!(list_val["id"], 2);

    let tools = list_val["result"]["tools"].as_array().expect("tools array");
    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    assert!(tool_names.contains(&"explore"));
    assert!(tool_names.contains(&"find_symbol"));
    assert!(tool_names.contains(&"impact"));
    assert!(tool_names.contains(&"callers"));
    assert!(tool_names.contains(&"callees"));
    assert!(tool_names.contains(&"cochange"));
    assert!(tool_names.contains(&"edit_check"));
    assert!(tool_names.contains(&"affected"));
    assert!(tool_names.contains(&"quality_baseline"));
    assert!(tool_names.contains(&"quality_delta"));

    // ── 4. TOOLS/CALL: explore ───────────────────────────────────────────────
    let explore_call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"explore","arguments":{"top_k":5}}}"#;
    let explore_resp =
        dispatch_json_rpc(explore_call, &mut state, ".").expect("must reply to explore");
    let explore_val: Value = serde_json::from_str(&explore_resp).unwrap();
    assert_eq!(explore_val["id"], 3);
    let text = explore_val["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(text.contains("<r schema=\"ripwire.map/v1\""));

    // ── 5. TOOLS/CALL: find_symbol ───────────────────────────────────────────
    let find_call = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"find_symbol","arguments":{"symbol":"run_cli"}}}"#;
    let find_resp =
        dispatch_json_rpc(find_call, &mut state, ".").expect("must reply to find_symbol");
    let find_val: Value = serde_json::from_str(&find_resp).unwrap();
    assert_eq!(find_val["id"], 4);
    let find_text = find_val["result"]["content"][0]["text"].as_str().unwrap();
    assert!(find_text.contains("run_cli") || find_text.contains("found"));

    // ── 6. TOOLS/CALL: callers & callees ─────────────────────────────────────
    let callers_call = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"callers","arguments":{"symbol":"run_cli"}}}"#;
    let callers_resp =
        dispatch_json_rpc(callers_call, &mut state, ".").expect("must reply to callers");
    let callers_val: Value = serde_json::from_str(&callers_resp).unwrap();
    assert_eq!(callers_val["id"], 5);

    let callees_call = r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"callees","arguments":{"symbol":"run_cli"}}}"#;
    let callees_resp =
        dispatch_json_rpc(callees_call, &mut state, ".").expect("must reply to callees");
    let callees_val: Value = serde_json::from_str(&callees_resp).unwrap();
    assert_eq!(callees_val["id"], 6);

    // ── 7. TOOLS/CALL: impact ────────────────────────────────────────────────
    let impact_call = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"impact","arguments":{"symbol":"run_cli","max_depth":3}}}"#;
    let impact_resp =
        dispatch_json_rpc(impact_call, &mut state, ".").expect("must reply to impact");
    let impact_val: Value = serde_json::from_str(&impact_resp).unwrap();
    assert_eq!(impact_val["id"], 7);

    // ── 8. ERROR CASES ───────────────────────────────────────────────────────
    let unknown_tool =
        r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"unknown_tool"}}"#;
    let err_resp = dispatch_json_rpc(unknown_tool, &mut state, ".").expect("must reply error");
    let err_val: Value = serde_json::from_str(&err_resp).unwrap();
    assert!(
        err_val["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown tool")
    );

    let unknown_method = r#"{"jsonrpc":"2.0","id":9,"method":"random_method"}"#;
    let method_err = dispatch_json_rpc(unknown_method, &mut state, ".").expect("must reply error");
    let method_val: Value = serde_json::from_str(&method_err).unwrap();
    assert_eq!(method_val["error"]["code"], -32601);
}

#[test]
fn test_mcp_stream_execution() {
    let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n";
    let cursor = Cursor::new(input);
    let mut out = Vec::new();

    run_mcp_stream(cursor, &mut out, ".").expect("stream run succeeds");
    let out_str = String::from_utf8(out).expect("valid utf8");
    assert!(out_str.contains("\"protocolVersion\":\"2024-11-05\""));
    assert!(out_str.contains("\"tools\""));
}

#[test]
fn test_cli_execution_modes() {
    // In-memory test fixture: zero disk I/O, zero network
    let fixtures: &[(&str, Lang, &[u8])] = &[(
        "lib.rs",
        Lang::Rust,
        b"pub fn run_cli() { helper(); } fn helper() {}",
    )];
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(fixtures, &reg, &interner);
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&PageRankConfig::default());

    let mut out = Vec::new();

    // 1. Stream XML
    assert!(execute_stream_xml_with(".", &ingest, &graph, &pr, &interner, &mut out).is_ok());
    assert!(!out.is_empty());

    // 2. Navigate with callers, callees, impact, and standard
    let nav_modes = [
        NavigateArgs {
            path: None,
            symbol: Some("run_cli".into()),
            callers: true,
            callees: false,
            impact: false,
        },
        NavigateArgs {
            path: None,
            symbol: Some("run_cli".into()),
            callers: false,
            callees: true,
            impact: false,
        },
        NavigateArgs {
            path: None,
            symbol: Some("run_cli".into()),
            callers: false,
            callees: false,
            impact: true,
        },
        NavigateArgs {
            path: None,
            symbol: Some("run_cli".into()),
            callers: false,
            callees: false,
            impact: false,
        },
        NavigateArgs {
            path: None,
            symbol: None,
            callers: false,
            callees: false,
            impact: false,
        },
        NavigateArgs {
            path: None,
            symbol: Some("nonexistent_symbol_404".into()),
            callers: false,
            callees: false,
            impact: false,
        },
    ];

    let ctx = ripwire::cli::NavContext {
        ingest: &ingest,
        graph: &graph,
        ranks: &pr.ranks,
        interner: &interner,
    };

    for args in &nav_modes {
        let mut nav_out = Vec::new();
        assert!(execute_navigate_with(args, &ctx, &mut nav_out).is_ok());
    }

    let mut callers_buf = Vec::new();
    execute_navigate_with(&nav_modes[0], &ctx, &mut callers_buf).unwrap();
    let callers_xml = String::from_utf8(callers_buf).unwrap();
    assert!(callers_xml.contains("<callers schema=\"ripwire.callers/v1\" of=\"run_cli\""));

    let mut callees_buf = Vec::new();
    execute_navigate_with(&nav_modes[1], &ctx, &mut callees_buf).unwrap();
    let callees_xml = String::from_utf8(callees_buf).unwrap();
    assert!(callees_xml.contains("<callees schema=\"ripwire.callees/v1\" of=\"run_cli\""));

    let mut impact_buf = Vec::new();
    execute_navigate_with(&nav_modes[2], &ctx, &mut impact_buf).unwrap();
    let impact_xml = String::from_utf8(impact_buf).unwrap();
    assert!(impact_xml.contains("<impact schema=\"ripwire.impact/v1\" of=\"run_cli\""));

    let mut sym_buf = Vec::new();
    execute_navigate_with(&nav_modes[3], &ctx, &mut sym_buf).unwrap();
    let sym_xml = String::from_utf8(sym_buf).unwrap();
    assert!(sym_xml.contains("<find-symbol schema=\"ripwire.find-symbol/v1\" of=\"run_cli\""));

    let mut uses_buf = Vec::new();
    ripwire::serialize_uses_xml("helper", &ingest, &interner, &mut uses_buf).unwrap();
    let uses_xml = String::from_utf8(uses_buf).unwrap();
    assert!(uses_xml.contains("<uses schema=\"ripwire.uses/v1\" of=\"helper\""));

    // 3. Subcommand stubs
    assert!(
        run_cli_args(Cli {
            command: Some(Commands::Gate(GateArgs::default())),
            ..Default::default()
        })
        .is_ok()
    );
    assert!(
        run_cli_args(Cli {
            command: Some(Commands::Metrics(MetricsArgs::default())),
            ..Default::default()
        })
        .is_ok()
    );
    assert!(
        run_cli_args(Cli {
            command: Some(Commands::Slice(SliceArgs::default())),
            ..Default::default()
        })
        .is_ok()
    );
}

#[test]
fn test_top_nav_queries() {
    let fixtures: &[(&str, Lang, &[u8])] = &[(
        "lib.rs",
        Lang::Rust,
        b"pub fn run_cli() { helper(); } fn helper() {}",
    )];
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(fixtures, &reg, &interner);
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&PageRankConfig::default());

    let modes = [
        (
            NavMode::Callers,
            "<callers schema=\"ripwire.callers/v1\" of=\"run_cli\"",
        ),
        (
            NavMode::Callees,
            "<callees schema=\"ripwire.callees/v1\" of=\"run_cli\"",
        ),
        (
            NavMode::Impact,
            "<impact schema=\"ripwire.impact/v1\" of=\"run_cli\"",
        ),
        (
            NavMode::Uses,
            "<uses schema=\"ripwire.uses/v1\" of=\"run_cli\"",
        ),
        (
            NavMode::FindSymbol,
            "<find-symbol schema=\"ripwire.find-symbol/v1\" of=\"run_cli\"",
        ),
    ];
    for (mode, expected) in modes {
        let mut buf = Vec::new();
        execute_nav_symbol_query(
            "run_cli", mode, &ingest, &graph, &pr.ranks, &interner, &mut buf,
        )
        .unwrap();
        let xml = String::from_utf8(buf).unwrap();
        assert!(xml.contains(expected));
    }

    let mut err_buf = Vec::new();
    let err = execute_nav_symbol_query(
        "missing_sym_404",
        NavMode::Callers,
        &ingest,
        &graph,
        &pr.ranks,
        &interner,
        &mut err_buf,
    );
    assert!(err.is_err());
    let err_msg = err.unwrap_err().to_string();
    assert!(err_msg.contains("ripwire: --callers symbol not found: missing_sym_404"));
}

fn setup_in_memory_fixture() -> (ripwire::domain::IngestResult, Graph, Interner) {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[(
            "lib.rs",
            Lang::Rust,
            b"pub fn run_cli() { helper(); } fn helper() {}",
        )],
        &reg,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    (ingest, graph, interner)
}

#[test]
fn test_top_retrieval_and_workspace_queries() {
    use ripwire::cli::{
        TopRetrievalArgs, TopWorkspaceArgs, execute_connect_query, execute_path_query,
        execute_retrieval_dispatch, execute_workspace_doc_query,
    };
    let (ingest, graph, interner) = setup_in_memory_fixture();

    let mut out = Vec::new();
    assert!(execute_connect_query("run_cli,helper", &ingest, &graph, &interner, &mut out).is_ok());
    assert!(execute_connect_query("missing_one", &ingest, &graph, &interner, &mut out).is_err());
    assert!(execute_path_query("run_cli,helper", &ingest, &graph, &interner, &mut out).is_ok());
    assert!(execute_path_query("missing,helper", &ingest, &graph, &interner, &mut out).is_err());

    let chk = |w, d, f, m| TopWorkspaceArgs {
        whereis: w,
        doc_drift: d,
        flags: f,
        mentions: m,
        ..Default::default()
    };
    assert!(
        execute_workspace_doc_query(
            &chk(Some("run_cli".into()), false, false, None),
            &ingest,
            &interner,
            &mut out
        )
        .is_ok()
    );
    assert!(
        execute_workspace_doc_query(
            &chk(None, true, true, Some("run_cli".into())),
            &ingest,
            &interner,
            &mut out
        )
        .is_ok()
    );
    assert!(
        execute_workspace_doc_query(
            &chk(None, false, false, Some("missing".into())),
            &ingest,
            &interner,
            &mut out
        )
        .is_err()
    );
    let pr = ripwire::PageRankResult {
        iterations: 1,
        converged: true,
        ranks: vec![0.5, 0.5],
    };
    let r_for = TopRetrievalArgs {
        task_for: Some("run".into()),
        ..Default::default()
    };
    assert!(
        execute_retrieval_dispatch(&r_for, ".", &ingest, &graph, &pr, &interner, &mut out).is_ok()
    );
    let r_grep = TopRetrievalArgs {
        grep: Some("fn".into()),
        ..Default::default()
    };
    assert!(
        execute_retrieval_dispatch(&r_grep, ".", &ingest, &graph, &pr, &interner, &mut out).is_ok()
    );
    let r_ex = TopRetrievalArgs {
        exemplar: Some("fn".into()),
        ..Default::default()
    };
    assert!(
        execute_retrieval_dispatch(&r_ex, ".", &ingest, &graph, &pr, &interner, &mut out).is_ok()
    );
    let r_lego = TopRetrievalArgs {
        lego: Some("Iface".into()),
        ..Default::default()
    };
    assert!(
        execute_retrieval_dispatch(&r_lego, ".", &ingest, &graph, &pr, &interner, &mut out).is_ok()
    );
    let r_rec = TopRetrievalArgs {
        recall: Some("plan".into()),
        ..Default::default()
    };
    assert!(
        execute_retrieval_dispatch(&r_rec, ".", &ingest, &graph, &pr, &interner, &mut out).is_ok()
    );
    let r_tr = TopRetrievalArgs {
        from_trace: Some("error at src/lib.rs:1".into()),
        ..Default::default()
    };
    assert!(
        execute_retrieval_dispatch(&r_tr, ".", &ingest, &graph, &pr, &interner, &mut out).is_ok()
    );
}

#[test]
fn test_top_safety_queries() {
    use ripwire::cli::{
        TopSafetyArgs, execute_exercises_query, execute_safe_delete_query, execute_safety_dispatch,
        execute_situ_query, execute_test_gate_query,
    };
    let (ingest, graph, interner) = setup_in_memory_fixture();
    let mut out = Vec::new();

    assert!(
        execute_test_gate_query(".", Some("lib.rs"), &ingest, &graph, &interner, &mut out).is_ok()
    );
    assert!(execute_situ_query(".", Some("lib.rs"), &ingest, &graph, &interner, &mut out).is_ok());
    assert!(
        execute_safe_delete_query(".", "run_cli", &ingest, &graph, &interner, &mut out).is_ok()
    );
    assert!(
        execute_safe_delete_query(".", "missing", &ingest, &graph, &interner, &mut out).is_err()
    );
    assert!(execute_exercises_query(".", "tests", &ingest, &graph, &interner, &mut out).is_ok());

    let s = |tg, si, sd, ex| TopSafetyArgs {
        test_gate: tg,
        situ: si,
        safe_delete: sd,
        exercises: ex,
    };
    assert!(
        execute_safety_dispatch(
            &s(Some(Some("lib.rs".into())), None, None, None),
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out
        )
        .is_ok()
    );
    assert!(
        execute_safety_dispatch(
            &s(None, Some(Some("lib.rs".into())), None, None),
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out
        )
        .is_ok()
    );
    assert!(
        execute_safety_dispatch(
            &s(None, None, Some("run_cli".into()), None),
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out
        )
        .is_ok()
    );
    assert!(
        execute_safety_dispatch(
            &s(None, None, None, Some("tests".into())),
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out
        )
        .is_ok()
    );
}
#[test]
fn test_top_architecture_queries() {
    use ripwire::cli::{
        AroundConfig, TopArchitectureArgs, execute_around_query, execute_batch_content,
        execute_communities_query, execute_dead_code_query, execute_deps_query,
        execute_external_surface_query,
    };
    let (ingest, graph, interner) = setup_in_memory_fixture();
    let mut out = Vec::new();

    assert!(execute_dead_code_query(".", &ingest, &graph, &interner, &mut out).is_ok());
    assert!(execute_communities_query(".", None, &ingest, &graph, &interner, &mut out).is_ok());
    let cfg = AroundConfig {
        depth: 1,
        fanout: 32,
    };
    assert!(
        execute_around_query(".", "run_cli", cfg, &ingest, &graph, &interner, &mut out).is_ok()
    );
    assert!(
        execute_around_query(".", "missing", cfg, &ingest, &graph, &interner, &mut out).is_err()
    );
    assert!(execute_external_surface_query(".", &ingest, &interner, &mut out).is_ok());
    assert!(execute_deps_query(".", &ingest, &graph, &interner, &mut out).is_ok());

    let pr = ripwire::PageRankResult {
        iterations: 1,
        converged: true,
        ranks: vec![0.5, 0.5],
    };
    let batch =
        "callers:run_cli\ncallees:run_cli\nimpact:run_cli\nuses:run_cli\ngrep:fn\nfor:run\n";
    assert!(execute_batch_content(".", batch, &ingest, &graph, &pr, &interner, &mut out).is_ok());

    let _a = TopArchitectureArgs {
        dead_code: true,
        ..Default::default()
    };
}
