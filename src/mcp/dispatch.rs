use crate::query::commands::dispatch;
use crate::workspace::Workspace;
use serde_json::{Value, json};

pub fn dispatch_tool_action(
    state: &Workspace,
    tool_name: &str,
    args: &Value,
) -> Result<Value, String> {
    let sources = crate::query::FileSources {
        root: std::path::Path::new(&state.root),
    };
    dispatch(&state.query_context(&sources), tool_name, args)
}

#[must_use]
pub fn handle_tools_call(
    id: &Value,
    params: &Value,
    state: &mut Option<Workspace>,
    default_root: &str,
) -> Value {
    let Some(tool_name) = params["name"].as_str() else {
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32602, "message": "missing tool 'name'" }
        });
    };

    if state.is_none() {
        *state = Workspace::load(default_root);
    }
    let Some(current_state) = state.as_mut() else {
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32603, "message": "failed to index workspace root" }
        });
    };

    let empty_args = json!({});
    let args = params.get("arguments").unwrap_or(&empty_args);
    match dispatch_tool_action(current_state, tool_name, args) {
        Ok(val) => {
            let text = match val {
                Value::String(s) => s,
                other => other.to_string(),
            };
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": text }]
                }
            })
        }
        Err(err_msg) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32602, "message": err_msg }
        }),
    }
}
