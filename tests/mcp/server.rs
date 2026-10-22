use ripwire_rs::mcp::{
    dispatch_lsp_json_rpc, run_json_rpc_stream_with_state, run_lsp_stream_with_state,
    run_mcp_stream_with_state,
};

use ripwire_rs::{Interner, Lang, LanguageRegistry, Workspace, ingest_in_memory_files};
use serde_json::{Value, json};

fn dispatch_json_rpc(line: &str, state: &mut Option<Workspace>, root: &str) -> Option<String> {
    ripwire_rs::mcp::dispatch_json_rpc(line, state, root, ripwire_rs::commands::tools())
}

use std::io::Cursor;

fn workspace(files: &[(&str, Lang, &[u8])]) -> Option<Workspace> {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(files, &registry, &interner);
    Some(Workspace::from_ingest(":memory:", ingest, interner))
}

#[test]
fn protocol_errors_preserve_request_identity_and_notifications_do_not_reply() {
    let mut state = workspace(&[]);
    let parse: Value =
        serde_json::from_str(&dispatch_json_rpc("{", &mut state, ":memory:").unwrap()).unwrap();
    assert_eq!(parse["error"]["code"], -32700);
    assert!(parse["id"].is_null());
    let unknown: Value = serde_json::from_str(
        &dispatch_json_rpc(
            r#"{"jsonrpc":"2.0","id":"request-7","method":"unsupported"}"#,
            &mut state,
            ":memory:",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(unknown["id"], "request-7");
    assert_eq!(unknown["error"]["code"], -32601);
    assert!(
        dispatch_json_rpc(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &mut state,
            ":memory:",
        )
        .is_none()
    );
    let bad_call: Value = serde_json::from_str(
        &dispatch_json_rpc(
            r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"callers","arguments":{"symbol":42}}}"#,
            &mut state,
            ":memory:",
        ).unwrap(),
    ).unwrap();
    assert_eq!(bad_call["id"], 8);
    assert_eq!(bad_call["error"]["code"], -32602);
    assert!(bad_call.get("result").is_none());
}

#[test]
fn lsp_locations_use_zero_based_lines_and_keep_reference_direction() {
    let mut state = workspace(&[(
        "src/lib.rs",
        Lang::Rust,
        b"pub fn alpha() {\n    beta();\n}\npub fn beta() {}\n",
    )]);
    let mut request = json!({
        "jsonrpc": "2.0", "id": 1, "method": "textDocument/definition",
        "params": {"textDocument": {"uri": "file://src/lib.rs"},
                   "position": {"line": 0, "character": 7}}
    });
    let definition: Value = serde_json::from_str(
        &dispatch_lsp_json_rpc(&request.to_string(), &mut state, ":memory:").unwrap(),
    )
    .unwrap();
    assert_eq!(definition["result"][0]["range"]["start"]["line"], 0);
    assert_eq!(definition["result"][0]["range"]["end"]["line"], 2);
    request["method"] = json!("textDocument/references");
    request["params"]["position"]["line"] = json!(3);
    let references: Value = serde_json::from_str(
        &dispatch_lsp_json_rpc(&request.to_string(), &mut state, ":memory:").unwrap(),
    )
    .unwrap();
    let sites: Vec<_> = references["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["uri"].as_str().unwrap(),
                row["range"]["start"]["line"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(sites, [("file://src/lib.rs", 1)]);
    request["method"] = json!("textDocument/documentSymbol");
    let symbols: Value = serde_json::from_str(
        &dispatch_lsp_json_rpc(&request.to_string(), &mut state, ":memory:").unwrap(),
    )
    .unwrap();
    let declarations: Vec<_> = symbols["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["name"].as_str().unwrap(),
                row["location"]["range"]["start"]["line"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(declarations, [("alpha", 0), ("beta", 3)]);
}

#[test]
fn lsp_lifecycle_hover_and_error_handling() {
    let mut state = workspace(&[(
        "src/lib.rs",
        Lang::Rust,
        b"pub fn alpha() {\n    beta();\n}\npub fn beta() {}\n",
    )]);

    // Malformed JSON and notification without id produce no response
    assert!(dispatch_lsp_json_rpc("not a valid json", &mut state, ":memory:").is_none());
    assert!(
        dispatch_lsp_json_rpc(
            r#"{"jsonrpc":"2.0","method":"initialized"}"#,
            &mut state,
            ":memory:",
        )
        .is_none()
    );

    // Initialize lifecycle request
    let init_resp: Value = serde_json::from_str(
        &dispatch_lsp_json_rpc(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            &mut state,
            ":memory:",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "ripwire-rs-lsp");
    assert_eq!(
        init_resp["result"]["capabilities"]["definitionProvider"],
        true
    );
    assert_eq!(
        init_resp["result"]["capabilities"]["referencesProvider"],
        true
    );
    assert_eq!(
        init_resp["result"]["capabilities"]["documentSymbolProvider"],
        true
    );
    assert_eq!(init_resp["result"]["capabilities"]["hoverProvider"], true);

    // Shutdown request returns null result
    let shutdown_resp: Value = serde_json::from_str(
        &dispatch_lsp_json_rpc(
            r#"{"jsonrpc":"2.0","id":"shut-42","method":"shutdown"}"#,
            &mut state,
            ":memory:",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(shutdown_resp["id"], "shut-42");
    assert!(shutdown_resp["result"].is_null());

    // Unknown method produces standard method-not-found error
    let unknown_resp: Value = serde_json::from_str(
        &dispatch_lsp_json_rpc(
            r#"{"jsonrpc":"2.0","id":99,"method":"textDocument/unknownAction","params":{}}"#,
            &mut state,
            ":memory:",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(unknown_resp["id"], 99);
    assert_eq!(unknown_resp["error"]["code"], -32601);
    assert_eq!(
        unknown_resp["error"]["message"],
        "Method not found: textDocument/unknownAction"
    );

    // Hover on alpha verifies caller/callee facts and markdown format
    let hover_alpha: Value = serde_json::from_str(
        &dispatch_lsp_json_rpc(
            &json!({
                "jsonrpc": "2.0",
                "id": 10,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file://src/lib.rs" },
                    "position": { "line": 0, "character": 7 }
                }
            })
            .to_string(),
            &mut state,
            ":memory:",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(hover_alpha["id"], 10);
    let alpha_doc = hover_alpha["result"]["contents"]["value"].as_str().unwrap();
    assert!(alpha_doc.contains("**alpha**"));
    assert!(alpha_doc.contains("callers: 0"));
    assert!(alpha_doc.contains("callees: 1"));

    // Hover on beta verifies caller/callee facts
    let hover_beta: Value = serde_json::from_str(
        &dispatch_lsp_json_rpc(
            &json!({
                "jsonrpc": "2.0",
                "id": 11,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file://src/lib.rs" },
                    "position": { "line": 3, "character": 7 }
                }
            })
            .to_string(),
            &mut state,
            ":memory:",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(hover_beta["id"], 11);
    let beta_doc = hover_beta["result"]["contents"]["value"].as_str().unwrap();
    assert!(beta_doc.contains("**beta**"));
    assert!(beta_doc.contains("callers: 1"));
    assert!(beta_doc.contains("callees: 0"));

    // Hover, definition, and references on unknown position
    let hover_none: Value = serde_json::from_str(
        &dispatch_lsp_json_rpc(
            &json!({
                "jsonrpc": "2.0",
                "id": 12,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": { "uri": "file://src/lib.rs" },
                    "position": { "line": 99, "character": 0 }
                }
            })
            .to_string(),
            &mut state,
            ":memory:",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(hover_none["id"], 12);
    assert!(hover_none["result"].is_null());

    let def_none: Value = serde_json::from_str(
        &dispatch_lsp_json_rpc(
            &json!({
                "jsonrpc": "2.0",
                "id": 13,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": { "uri": "file://src/lib.rs" },
                    "position": { "line": 99, "character": 0 }
                }
            })
            .to_string(),
            &mut state,
            ":memory:",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(def_none["id"], 13);
    assert_eq!(def_none["result"], json!([]));

    let ref_none: Value = serde_json::from_str(
        &dispatch_lsp_json_rpc(
            &json!({
                "jsonrpc": "2.0",
                "id": 14,
                "method": "textDocument/references",
                "params": {
                    "textDocument": { "uri": "file://src/lib.rs" },
                    "position": { "line": 99, "character": 0 }
                }
            })
            .to_string(),
            &mut state,
            ":memory:",
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(ref_none["id"], 14);
    assert_eq!(ref_none["result"], json!([]));
}

#[test]
fn json_rpc_stream_with_state_handles_multiple_frames_headers_and_notifications() {
    let mut state = workspace(&[(
        "src/lib.rs",
        Lang::Rust,
        b"pub fn alpha() {\n    beta();\n}\npub fn beta() {}\n",
    )]);

    // LSP stream with Content-Length headers, empty lines, notifications, and multiple requests
    let lsp_input = concat!(
        "Content-Length: 128\r\n",
        "\r\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"initialized\"}\n",
        "   \n",
        "{\"jsonrpc\":\"2.0\",\"id\":200,\"method\":\"shutdown\"}\n",
        "Content-Length: 256\n",
        "{\"jsonrpc\":\"2.0\",\"id\":201,\"method\":\"textDocument/definition\",\"params\":{\"textDocument\":{\"uri\":\"file://src/lib.rs\"},\"position\":{\"line\":0,\"character\":7}}}\n"
    );
    let mut lsp_output = Vec::new();
    run_lsp_stream_with_state(
        Cursor::new(lsp_input),
        &mut lsp_output,
        ":memory:",
        &mut state,
    )
    .unwrap();

    let lsp_output_str = String::from_utf8(lsp_output).unwrap();
    let lsp_lines: Vec<&str> = lsp_output_str.trim().lines().collect();
    assert_eq!(lsp_lines.len(), 2);

    let resp1: Value = serde_json::from_str(lsp_lines[0]).unwrap();
    assert_eq!(resp1["id"], 200);
    assert!(resp1["result"].is_null());

    let resp2: Value = serde_json::from_str(lsp_lines[1]).unwrap();
    assert_eq!(resp2["id"], 201);
    assert_eq!(resp2["result"][0]["uri"], "file://src/lib.rs");
    assert_eq!(resp2["result"][0]["range"]["start"]["line"], 0);
    assert_eq!(resp2["result"][0]["range"]["end"]["line"], 2);

    // MCP stream with notifications, resource listing, malformed input, and unknown method
    let mcp_input = concat!(
        "Content-Length: 64\r\n",
        "\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-res\",\"method\":\"resources/list\"}\n",
        "not-a-json-line\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-unknown\",\"method\":\"nosuchmethod\"}\n"
    );
    let mut mcp_output = Vec::new();
    let mut mcp_state = workspace(&[]);
    run_mcp_stream_with_state(
        Cursor::new(mcp_input),
        &mut mcp_output,
        ":memory:",
        &mut mcp_state,
        ripwire_rs::commands::tools(),
    )
    .unwrap();

    let mcp_output_str = String::from_utf8(mcp_output).unwrap();
    let mcp_lines: Vec<&str> = mcp_output_str.trim().lines().collect();
    assert_eq!(mcp_lines.len(), 3);

    let res_list: Value = serde_json::from_str(mcp_lines[0]).unwrap();
    assert_eq!(res_list["id"], "req-res");
    assert!(
        res_list["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["uri"] == "ripwire-rs://legend-dict/full")
    );

    let parse_err: Value = serde_json::from_str(mcp_lines[1]).unwrap();
    assert_eq!(parse_err["error"]["code"], -32700);

    let unk_err: Value = serde_json::from_str(mcp_lines[2]).unwrap();
    assert_eq!(unk_err["id"], "req-unknown");
    assert_eq!(unk_err["error"]["code"], -32601);

    // Direct invocation of run_json_rpc_stream_with_state
    let direct_input = "{\"jsonrpc\":\"2.0\",\"id\":300,\"method\":\"shutdown\"}\n";
    let mut direct_output = Vec::new();
    run_json_rpc_stream_with_state(
        Cursor::new(direct_input),
        &mut direct_output,
        ":memory:",
        &mut state,
        dispatch_lsp_json_rpc,
    )
    .unwrap();
    let direct_resp: Value =
        serde_json::from_str(String::from_utf8(direct_output).unwrap().trim()).unwrap();
    assert_eq!(direct_resp["id"], 300);
    assert!(direct_resp["result"].is_null());
}
