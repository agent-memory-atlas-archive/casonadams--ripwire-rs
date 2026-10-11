use rwire::cli::{GateArgs, MetricsArgs, NavigateArgs, OrientArgs, SliceArgs};
use rwire::{dispatch_json_rpc, run_cli_args, run_mcp_stream, Cli, Commands};
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
    let mut state = None;

    // ── 1. INITIALIZE ────────────────────────────────────────────────────────
    let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#;
    let init_resp = dispatch_json_rpc(init_req, &mut state, ".").expect("must reply to initialize");
    let init_val: Value = serde_json::from_str(&init_resp).unwrap();
    assert_eq!(init_val["id"], 1);
    assert_eq!(init_val["result"]["serverInfo"]["name"], "rwire");
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

    // ── 4. TOOLS/CALL: explore ───────────────────────────────────────────────
    let explore_call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"explore","arguments":{"top_k":5}}}"#;
    let explore_resp =
        dispatch_json_rpc(explore_call, &mut state, ".").expect("must reply to explore");
    let explore_val: Value = serde_json::from_str(&explore_resp).unwrap();
    assert_eq!(explore_val["id"], 3);
    let text = explore_val["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(text.contains("<r schema=\"rwire.map/v1\""));

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
    assert!(err_val["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown tool"));

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
    // 1. Default run
    assert!(run_cli_args(Cli {
        path: Some(".".into()),
        command: None,
    })
    .is_ok());

    // 2. Orient
    assert!(run_cli_args(Cli {
        path: None,
        command: Some(Commands::Orient(OrientArgs {
            path: Some(".".into()),
            for_task: Some("testing".into()),
        })),
    })
    .is_ok());

    // 3. Navigate with callers, callees, impact, and standard
    let nav_modes = [
        NavigateArgs {
            path: Some(".".into()),
            symbol: Some("run_cli".into()),
            callers: true,
            callees: false,
            impact: false,
        },
        NavigateArgs {
            path: Some(".".into()),
            symbol: Some("run_cli".into()),
            callers: false,
            callees: true,
            impact: false,
        },
        NavigateArgs {
            path: Some(".".into()),
            symbol: Some("run_cli".into()),
            callers: false,
            callees: false,
            impact: true,
        },
        NavigateArgs {
            path: Some(".".into()),
            symbol: Some("run_cli".into()),
            callers: false,
            callees: false,
            impact: false,
        },
        NavigateArgs {
            path: Some(".".into()),
            symbol: None,
            callers: false,
            callees: false,
            impact: false,
        },
        NavigateArgs {
            path: Some(".".into()),
            symbol: Some("nonexistent_symbol_404".into()),
            callers: false,
            callees: false,
            impact: false,
        },
    ];

    for args in nav_modes {
        assert!(run_cli_args(Cli {
            path: None,
            command: Some(Commands::Navigate(args)),
        })
        .is_ok());
    }

    // 4. Stubs for future waves
    assert!(run_cli_args(Cli {
        path: None,
        command: Some(Commands::Gate(GateArgs::default())),
    })
    .is_ok());
    assert!(run_cli_args(Cli {
        path: None,
        command: Some(Commands::Metrics(MetricsArgs::default())),
    })
    .is_ok());
    assert!(run_cli_args(Cli {
        path: None,
        command: Some(Commands::Slice(SliceArgs::default())),
    })
    .is_ok());
}
