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
}
