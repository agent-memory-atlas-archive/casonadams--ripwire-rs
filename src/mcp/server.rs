use super::dispatch::handle_tools_call;
use super::schema;
use super::state::McpState;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

/// Handle `initialize` method.
#[must_use]
pub fn handle_initialize(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "ripwire",
                "version": "0.1.0"
            },
            "capabilities": {
                "tools": {}
            }
        }
    })
}

/// Handle `tools/list` method exposing all 33 MCP tools.
#[must_use]
pub fn handle_tools_list(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": schema::all_tool_definitions()
        }
    })
}

/// Run the stdio JSON-RPC loop for MCP clients.
pub fn run_mcp_stdio(root: &str) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_mcp_stream(stdin.lock(), stdout.lock(), root)
}

/// Run MCP JSON-RPC protocol over any Read/Write stream pair.
pub fn run_mcp_stream<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    root: &str,
) -> io::Result<()> {
    let mut state = McpState::load(root);

    for line_res in reader.lines() {
        let line = line_res?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = dispatch_json_rpc(trimmed, &mut state, root) {
            writeln!(writer, "{response}")?;
            writer.flush()?;
        }
    }
    Ok(())
}

/// Dispatch a single raw JSON-RPC line. Returns `None` for notifications.
#[must_use]
pub fn dispatch_json_rpc(
    line: &str,
    state: &mut Option<McpState>,
    default_root: &str,
) -> Option<String> {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => {
            let err = json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": { "code": -32700, "message": "Parse error" }
            });
            return Some(err.to_string());
        }
    };

    let id = req.get("id")?;

    let resp = match req["method"].as_str().unwrap_or("") {
        "initialize" => handle_initialize(id),
        "tools/list" => handle_tools_list(id),
        "tools/call" => {
            let params = req.get("params").unwrap_or(&Value::Null);
            handle_tools_call(id, params, state, default_root)
        }
        method => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {method}") }
        }),
    };

    Some(resp.to_string())
}
