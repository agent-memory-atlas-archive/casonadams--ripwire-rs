use crate::query::commands::all_commands;
use serde_json::{Map, Value, json};

/// Build an MCP tool JSON Schema object from declarative parameter tuples.
#[must_use]
pub fn build_tool_schema_json(
    name: &str,
    description: &str,
    params: &[(&str, &str, &str)],
    required: &[&str],
) -> Value {
    let mut props = Map::new();
    for &(pname, ptype, pdesc) in params {
        let prop_val = if ptype == "string[]" {
            json!({
                "type": "array",
                "items": { "type": "string" },
                "description": pdesc
            })
        } else {
            json!({
                "type": ptype,
                "description": pdesc
            })
        };
        props.insert(pname.to_string(), prop_val);
    }
    let mut input_schema = Map::new();
    input_schema.insert("type".to_string(), Value::String("object".to_string()));
    input_schema.insert("properties".to_string(), Value::Object(props));
    if !required.is_empty() {
        input_schema.insert(
            "required".to_string(),
            Value::Array(
                required
                    .iter()
                    .map(|&r| Value::String(r.to_string()))
                    .collect(),
            ),
        );
    }
    json!({
        "name": name,
        "description": description,
        "inputSchema": Value::Object(input_schema)
    })
}

/// Render the shared command catalog as MCP JSON Schema.
#[must_use]
pub fn all_tool_definitions() -> Vec<Value> {
    all_commands()
        .into_iter()
        .map(|command| {
            build_tool_schema_json(
                command.name,
                command.description,
                &command.parameters().collect::<Vec<_>>(),
                command.required,
            )
        })
        .collect()
}
