use super::dispatch::handle_tools_call;
use super::schema;
use crate::query::commands::CommandDescriptor;
use crate::workspace::Workspace;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

/// Shown by MCP clients in the agent's system prompt.
pub const SERVER_INSTRUCTIONS: &str = "\
ripwire-rs answers structural questions about this repository from its parsed call graph.
- Who calls X, what X calls, what breaks if X changes: use `callers`, `callees` and `impact` before grep. \
Each row is the function or method containing the call site; same-named definitions in other classes or \
modules stay apart, and calls that could not be bound are listed as declined rather than guessed.
- Every call needs `symbol`. When several definitions share the name, pin one: owner-qualified \
(`Owner::method` or `Owner.method`), with `file` (`{\"symbol\": \"run\", \"file\": \"src/app.rs\"}`), or by \
the definition's location (`\"symbol\": \"src/app.rs:42\"`).
- Several questions at once: `batch` with `{\"queries\": [{\"tool\": \"callers\", \"symbol\": \"run\"}, ...]}`. \
Orientation in unfamiliar code: `explore`.
- grep stays the right tool for string literals, comments, configuration keys and other non-code text.";

#[must_use]
pub fn handle_initialize(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "ripwire-rs",
                "version": "0.1.0"
            },
            "capabilities": {
                "tools": {},
                "resources": {}
            },
            "instructions": SERVER_INSTRUCTIONS
        }
    })
}

#[must_use]
pub fn handle_tools_list(id: &Value, tools: &[&CommandDescriptor]) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": schema::all_tool_definitions(tools)
        }
    })
}

/// Run the stdio JSON-RPC loop for MCP clients.
pub fn run_mcp_stdio(root: &str, tools: &[&CommandDescriptor]) -> io::Result<()> {
    run_mcp_stdio_with_scip(root, None, tools)
}

/// Run MCP with a compiler-indexed workspace, failing before serving on index errors.
pub fn run_mcp_stdio_with_scip(
    root: &str,
    scip_index: Option<&std::path::Path>,
    tools: &[&CommandDescriptor],
) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let loaded = Workspace::load_with_scip(root, scip_index)?;
    let mut state = Some(loaded);
    run_json_rpc_stream_with_state(
        stdin.lock(),
        stdout.lock(),
        root,
        &mut state,
        |line, state, default_root| dispatch_json_rpc(line, state, default_root, tools),
    )
}

/// Run JSON-RPC protocol over any Read/Write stream pair with an existing workspace state.
pub fn run_json_rpc_stream_with_state<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    root: &str,
    state: &mut Option<Workspace>,
    dispatch: impl Fn(&str, &mut Option<Workspace>, &str) -> Option<String>,
) -> io::Result<()> {
    for line_res in reader.lines() {
        let line = line_res?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Content-Length:") {
            continue;
        }
        if let Some(resp) = dispatch(trimmed, state, root) {
            writeln!(writer, "{resp}")?;
            writer.flush()?;
        }
    }
    Ok(())
}

/// Run MCP JSON-RPC protocol over any Read/Write stream pair with an existing workspace state.
pub fn run_mcp_stream_with_state<R: BufRead, W: Write>(
    reader: R,
    writer: W,
    root: &str,
    state: &mut Option<Workspace>,
    tools: &[&CommandDescriptor],
) -> io::Result<()> {
    run_json_rpc_stream_with_state(reader, writer, root, state, |line, state, default_root| {
        dispatch_json_rpc(line, state, default_root, tools)
    })
}

/// Run MCP JSON-RPC protocol over any Read/Write stream pair.
pub fn run_mcp_stream<R: BufRead, W: Write>(
    reader: R,
    writer: W,
    root: &str,
    tools: &[&CommandDescriptor],
) -> io::Result<()> {
    let mut state = Workspace::load(root);
    run_mcp_stream_with_state(reader, writer, root, &mut state, tools)
}

/// Dispatch a single raw JSON-RPC line. Returns `None` for notifications.
#[must_use]
pub fn dispatch_json_rpc(
    line: &str,
    state: &mut Option<Workspace>,
    default_root: &str,
    tools: &[&CommandDescriptor],
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
        "tools/list" => handle_tools_list(id, tools),
        "resources/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "resources": [{
                    "uri": "ripwire-rs://legend-dict/full",
                    "name": "Ripwire Schema & Attribute Encyclopedia",
                    "mimeType": "application/xml"
                }]
            }
        }),
        "resources/read" => {
            let mut buf = Vec::new();
            let _ = crate::serialize::serialize_legend_dict_xml(&mut buf);
            let xml = String::from_utf8_lossy(&buf).into_owned();
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "contents": [{
                        "uri": "ripwire-rs://legend-dict/full",
                        "mimeType": "application/xml",
                        "text": xml
                    }]
                }
            })
        }
        "tools/call" => {
            let params = req.get("params").unwrap_or(&Value::Null);
            handle_tools_call(id, params, state, default_root, tools)
        }
        method => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {method}") }
        }),
    };

    Some(resp.to_string())
}

fn extract_lsp_file_and_line(params: &Value) -> (&str, u32) {
    let uri = params["textDocument"]["uri"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches("file://");
    let line = params["position"]["line"].as_u64().unwrap_or(0) as u32 + 1;
    (uri, line)
}

fn handle_lsp_definition(id: &Value, params: &Value, state: &Workspace) -> Value {
    let (uri, line) = extract_lsp_file_and_line(params);
    let Some(sym) = crate::navigate::resolve_symbol_at(uri, line, &state.ingest, &state.interner)
    else {
        return json!({ "jsonrpc": "2.0", "id": id, "result": [] });
    };
    let path = state
        .interner
        .resolve(&state.ingest.files[sym.file_id as usize].path);
    let start_line = sym.line.saturating_sub(1);
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": [{
            "uri": format!("file://{path}"),
            "range": {
                "start": { "line": start_line, "character": 0 },
                "end": { "line": start_line + sym.loc.max(1) - 1, "character": 0 }
            }
        }]
    })
}

fn handle_lsp_references(id: &Value, params: &Value, state: &Workspace) -> Value {
    let (uri, line) = extract_lsp_file_and_line(params);
    let Some(sym) = crate::navigate::resolve_symbol_at(uri, line, &state.ingest, &state.interner)
    else {
        return json!({ "jsonrpc": "2.0", "id": id, "result": [] });
    };
    use crate::domain::CorpusView;
    let target_name = state.sym_name(sym);
    let mut locations = Vec::new();
    for r in &state.ingest.references {
        let p = state.file_path(r.file_id);
        if state.reference_matches_query(r, target_name) && !p.is_empty() {
            let l = r.line.saturating_sub(1);
            locations.push(json!({
                "uri": format!("file://{p}"),
                "range": {
                    "start": { "line": l, "character": 0 },
                    "end": { "line": l, "character": 0 }
                }
            }));
        }
    }
    json!({ "jsonrpc": "2.0", "id": id, "result": locations })
}

fn handle_lsp_document_symbols(id: &Value, params: &Value, state: &Workspace) -> Value {
    let (uri, _) = extract_lsp_file_and_line(params);
    let mut symbols = Vec::new();
    for sym in &state.ingest.symbols {
        let p = state
            .interner
            .resolve(&state.ingest.files[sym.file_id as usize].path);
        if uri.is_empty() || p == uri || uri.ends_with(p) {
            let l = sym.line.saturating_sub(1);
            symbols.push(json!({
                "name": state.interner.resolve(&sym.name),
                "kind": 12,
                "location": {
                    "uri": format!("file://{p}"),
                    "range": {
                        "start": { "line": l, "character": 0 },
                        "end": { "line": l + sym.loc.max(1) - 1, "character": 0 }
                    }
                }
            }));
        }
    }
    json!({ "jsonrpc": "2.0", "id": id, "result": symbols })
}

fn handle_lsp_hover(id: &Value, params: &Value, state: &Workspace) -> Value {
    let (uri, line) = extract_lsp_file_and_line(params);
    let Some(sym) = crate::navigate::resolve_symbol_at(uri, line, &state.ingest, &state.interner)
    else {
        return json!({ "jsonrpc": "2.0", "id": id, "result": Value::Null });
    };
    let name = state.interner.resolve(&sym.name);
    let kind = sym.kind.tag();
    let in_deg = state.graph.in_degree(sym.id);
    let out_deg = state.graph.out_degree(sym.id);
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "contents": {
                "kind": "markdown",
                "value": format!("**{name}** (`{kind}`) — callers: {in_deg}, callees: {out_deg}, ccx: {}", sym.ccx)
            }
        }
    })
}

/// Dispatch a single LSP JSON-RPC request line.
#[must_use]
pub fn dispatch_lsp_json_rpc(
    line: &str,
    state: &mut Option<Workspace>,
    default_root: &str,
) -> Option<String> {
    let req: Value = serde_json::from_str(line).ok()?;
    let id = req.get("id")?;
    let method = req["method"].as_str().unwrap_or("");
    if method == "initialize" {
        return Some(
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "serverInfo": { "name": "ripwire-rs-lsp", "version": env!("CARGO_PKG_VERSION") },
                    "capabilities": {
                        "definitionProvider": true,
                        "referencesProvider": true,
                        "documentSymbolProvider": true,
                        "hoverProvider": true
                    }
                }
            })
            .to_string(),
        );
    }
    if method == "shutdown" {
        return Some(json!({ "jsonrpc": "2.0", "id": id, "result": Value::Null }).to_string());
    }
    if state.is_none() {
        *state = Workspace::load(default_root);
    }
    let Some(st) = state.as_ref() else {
        return Some(
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32603, "message": "Workspace not loaded" }
            })
            .to_string(),
        );
    };
    let params = req.get("params").unwrap_or(&Value::Null);
    let resp = match method {
        "textDocument/definition" => handle_lsp_definition(id, params, st),
        "textDocument/references" => handle_lsp_references(id, params, st),
        "textDocument/documentSymbol" => handle_lsp_document_symbols(id, params, st),
        "textDocument/hover" => handle_lsp_hover(id, params, st),
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {method}") }
        }),
    };
    Some(resp.to_string())
}

/// Run LSP JSON-RPC protocol over any Read/Write stream pair with an existing workspace state.
pub fn run_lsp_stream_with_state<R: BufRead, W: Write>(
    reader: R,
    writer: W,
    root: &str,
    state: &mut Option<Workspace>,
) -> io::Result<()> {
    run_json_rpc_stream_with_state(reader, writer, root, state, dispatch_lsp_json_rpc)
}

/// Run the stdio JSON-RPC Language Server Protocol loop over any Read/Write stream pair.
pub fn run_lsp_stream<R: BufRead, W: Write>(reader: R, writer: W, root: &str) -> io::Result<()> {
    let mut state = Workspace::load(root);
    run_lsp_stream_with_state(reader, writer, root, &mut state)
}

/// Run the stdio JSON-RPC Language Server Protocol loop.
pub fn run_lsp_stdio(root: &str) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_lsp_stream(stdin.lock(), stdout.lock(), root)
}
