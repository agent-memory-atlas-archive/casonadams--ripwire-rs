pub mod dispatch;
pub mod doc_tools;
pub mod edit;
pub mod graph_tools;
pub mod retrieval;
pub mod schema;
pub mod server;
pub mod state;

pub use dispatch::*;
pub use server::*;
pub use state::*;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_mcp_initialize_and_tools_list() {
        let id = json!(1);
        let init_resp = handle_initialize(&id);
        assert_eq!(init_resp["result"]["serverInfo"]["name"], "ripwire");

        let list_resp = handle_tools_list(&id);
        let tools = list_resp["result"]["tools"].as_array().unwrap();
        assert!(tools.len() >= 33, "Should list at least 33 tools");
        assert!(tools.iter().any(|t| t["name"] == "replace_symbol_body"));
        assert!(tools.iter().any(|t| t["name"] == "insert_before_symbol"));
        assert!(tools.iter().any(|t| t["name"] == "insert_after_symbol"));
        assert!(tools.iter().any(|t| t["name"] == "slice"));
        assert!(tools.iter().any(|t| t["name"] == "for"));
    }

    #[test]
    fn test_lsp_json_rpc_dispatch() {
        let reg = crate::ingest::LanguageRegistry::new().unwrap();
        let interner = crate::domain::Interner::new();
        let code = b"pub fn alpha() {\n    beta();\n}\npub fn beta() {}\n";
        let ingest = crate::ingest::ingest_in_memory_files(
            &[("src/lib.rs", crate::domain::Lang::Rust, code)],
            &reg,
            &interner,
        );
        let mut state = Some(McpState::from_ingest(".", ingest, interner));

        let init = dispatch_lsp_json_rpc(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            &mut state,
            ".",
        )
        .unwrap();
        assert!(init.contains("ripwire-lsp"));

        let def = dispatch_lsp_json_rpc(
            r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/definition","params":{"textDocument":{"uri":"file://src/lib.rs"},"position":{"line":0,"character":7}}}"#,
            &mut state,
            ".",
        )
        .unwrap();
        assert!(def.contains("file://src/lib.rs"));

        let refs = dispatch_lsp_json_rpc(
            r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/references","params":{"textDocument":{"uri":"file://src/lib.rs"},"position":{"line":3,"character":7}}}"#,
            &mut state,
            ".",
        )
        .unwrap();
        assert!(refs.contains("file://src/lib.rs"));

        let syms = dispatch_lsp_json_rpc(
            r#"{"jsonrpc":"2.0","id":4,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file://src/lib.rs"}}}"#,
            &mut state,
            ".",
        )
        .unwrap();
        assert!(syms.contains("alpha"));

        let hov = dispatch_lsp_json_rpc(
            r#"{"jsonrpc":"2.0","id":5,"method":"textDocument/hover","params":{"textDocument":{"uri":"file://src/lib.rs"},"position":{"line":0,"character":7}}}"#,
            &mut state,
            ".",
        )
        .unwrap();
        assert!(hov.contains("**alpha**"));
        let stream_in = std::io::Cursor::new(
            b"Content-Length: 55\n\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"shutdown\"}\n",
        );
        let mut stream_out = Vec::new();
        run_lsp_stream(stream_in, &mut stream_out, ".").unwrap();
        assert!(String::from_utf8_lossy(&stream_out).contains("ripwire-lsp"));
    }
}
