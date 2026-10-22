//! Builds the `CommandDescriptor` literal of one MCP tool, for a command module's `tools:` list.

/// `CommandDescriptor` literal: tool name, description, `(param, type, description)` rows, required params, handler.
macro_rules! mcp_tool {
    ($name:expr, $desc:expr, $params:expr, $req:expr, $handler:expr) => {
        &$crate::query::commands::CommandDescriptor {
            name: $name,
            description: $desc,
            params: $params,
            required: $req,
            handler: $handler,
        }
    };
}

pub(crate) use mcp_tool;
