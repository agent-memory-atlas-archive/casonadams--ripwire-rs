use ripwire_rs::mcp::{McpState, dispatch_json_rpc, handle_tools_list};
use ripwire_rs::{IngestResult, Interner};
use serde_json::{Value, json};

#[test]
fn test_all_33_mcp_tools_invocable() {
    let id = json!(42);
    let list_resp = handle_tools_list(&id);
    let tools = list_resp["result"]["tools"]
        .as_array()
        .expect("tools array");

    let all_33_names = [
        "affected",
        "analyze",
        "batch",
        "cochange",
        "connect",
        "doc_drift",
        "edit_check",
        "exemplar",
        "explore",
        "fetch_body",
        "find_referencing_symbols",
        "find_symbol",
        "flags",
        "for",
        "from_trace",
        "grep",
        "impact",
        "insert_after_symbol",
        "insert_before_symbol",
        "lego",
        "memory_recall",
        "mentions",
        "owners",
        "path_between",
        "quality_baseline",
        "quality_delta",
        "rank_by",
        "replace_symbol_body",
        "situational_awareness",
        "slice",
        "stray_content",
        "uses",
        "whereis",
    ];

    for name in all_33_names {
        assert!(
            tools.iter().any(|t| t["name"] == name),
            "Tool '{}' missing from tools/list",
            name
        );
    }

    let interner = Interner::new();
    let ingest = IngestResult::new();
    let mut state = Some(McpState::from_ingest(":memory:", ingest, interner));

    for name in all_33_names {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 100,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": {
                    "symbol": "main",
                    "target": "main:x",
                    "files": "src/lib.rs",
                    "pattern": "test",
                    "task": "build project",
                    "trace": "at main (src/main.rs:10)",
                    "interface": "Interface",
                    "from": "a",
                    "to": "b",
                    "rank_by": "pagerank",
                    "queries": []
                }
            }
        });
        let raw_resp = dispatch_json_rpc(&req.to_string(), &mut state, ":memory:")
            .expect("response from dispatch");
        let resp: Value = serde_json::from_str(&raw_resp).expect("valid json response");
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 100);
        assert!(
            resp.get("result").is_some() || resp.get("error").is_some(),
            "Tool '{}' returned invalid JSON-RPC response: {:?}",
            name,
            resp
        );
    }
}
