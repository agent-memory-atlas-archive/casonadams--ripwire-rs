use crate::crawler::{crawl_directory, CrawlerOptions};
use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::ingest::{ingest_crawled_files, LanguageRegistry};
use crate::navigate::{find_symbols_by_name, navigate_1hop, transitive_impact};
use crate::rank::{PageRankConfig, PageRankResult};
use crate::serialize::{serialize_xml_to_string, SerializeOptions};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

/// In-memory repository index state for fast repeated tool queries.
pub struct McpState {
    pub root: String,
    pub ingest: IngestResult,
    pub graph: Graph,
    pub ranks: Vec<f64>,
    pub interner: Interner,
    pub pr: PageRankResult,
    pub commits: Option<Vec<crate::git::history::CommitInfo>>,
}

impl McpState {
    #[must_use]
    pub fn load(root: &str) -> Option<Self> {
        let path = Path::new(root);
        let files = crawl_directory(path, &CrawlerOptions::default());
        let registry = LanguageRegistry::new_tier1().ok()?;
        let interner = Interner::new();
        let ingest = ingest_crawled_files(&files, &registry, &interner);
        let graph = Graph::build(&ingest, &interner);
        let pr = graph.page_rank(&PageRankConfig::default());
        let ranks = pr.ranks.clone();

        let commits = crate::git::history::walk_repository_history(path).ok();
        Some(Self {
            root: root.to_string(),
            ingest,
            graph,
            ranks,
            interner,
            pr,
            commits,
        })
    }

    #[must_use]
    pub fn from_ingest(root: &str, ingest: IngestResult, interner: Interner) -> Self {
        let graph = Graph::build(&ingest, &interner);
        let pr = graph.page_rank(&PageRankConfig::default());
        let ranks = pr.ranks.clone();
        Self {
            root: root.to_string(),
            ingest,
            graph,
            ranks,
            interner,
            pr,
            commits: None,
        }
    }

    #[must_use]
    pub fn with_commits(mut self, commits: Vec<crate::git::history::CommitInfo>) -> Self {
        self.commits = Some(commits);
        self
    }
}

/// JSON-RPC 2.0 Request envelope.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// Handle `initialize` method.
#[must_use]
pub fn handle_initialize(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "rwire",
                "version": "0.1.0"
            },
            "capabilities": {
                "tools": {}
            }
        }
    })
}

/// Handle `tools/list` method.
#[must_use]
pub fn handle_tools_list(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
                {
                    "name": "explore",
                    "description": "ONE-call task orientation: ranked symbols, files, and architecture map",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Repository path to explore" },
                            "top_k": { "type": "integer", "description": "Maximum number of symbols to include" }
                        }
                    }
                },
                {
                    "name": "find_symbol",
                    "description": "A symbol's 1-hop neighborhood: the symbol plus direct callers and callees",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string", "description": "Symbol name to look up" }
                        },
                        "required": ["symbol"]
                    }
                },
                {
                    "name": "callers",
                    "description": "Direct (1-hop) callers of a symbol",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string", "description": "Symbol name to look up" }
                        },
                        "required": ["symbol"]
                    }
                },
                {
                    "name": "callees",
                    "description": "Direct (1-hop) callees of a symbol",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string", "description": "Symbol name to look up" }
                        },
                        "required": ["symbol"]
                    }
                },
                {
                    "name": "impact",
                    "description": "Transitive blast radius of a symbol via calls, ranked by PageRank",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string", "description": "Symbol name to analyze blast radius for" },
                            "max_depth": { "type": "integer", "description": "Maximum hop depth" }
                        },
                        "required": ["symbol"]
                    }
                }
                ,
                {
                    "name": "cochange",
                    "description": "Files that historically change together with this file (Fowler's Shotgun Surgery logical coupling)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Target file path to find co-change partners for" },
                            "since": { "type": "string", "description": "Time window or revision (e.g. '18 months ago')" }
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "edit_check",
                    "description": "After an edit: did SYM's contract change, and which callers no longer fit?",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string", "description": "Symbol name to verify" }
                        },
                        "required": ["symbol"]
                    }
                },
                {
                    "name": "affected",
                    "description": "WHICH TESTS TO RUN for a change — test files that transitively reach the changed files/symbols",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "files": { "type": "string", "description": "Changed files or symbols, comma-separated" }
                        },
                        "required": ["files"]
                    }
                },
                {
                    "name": "quality_baseline",
                    "description": "PIN the quality floor: writes .rwire_quality_baseline stamped with git HEAD sha",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "quality_delta",
                    "description": "Your PR self-check: reports ONLY what your working tree made WORSE vs baseline",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                }
            ]
        }
    })
}

fn call_find_symbol(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = args["symbol"]
        .as_str()
        .ok_or_else(|| "missing 'symbol' parameter".to_string())?;
    let ids = find_symbols_by_name(sym_name, &state.ingest, &state.interner);
    if ids.is_empty() {
        return Ok(
            json!({ "found": false, "message": format!("symbol '{}' not found", sym_name) }),
        );
    }

    let neighborhoods: Vec<_> = ids
        .into_iter()
        .filter_map(|id| {
            navigate_1hop(
                id,
                &state.ingest,
                &state.graph,
                &state.ranks,
                &state.interner,
            )
        })
        .collect();

    Ok(json!({ "found": true, "matches": neighborhoods }))
}

fn call_callers(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = args["symbol"]
        .as_str()
        .ok_or_else(|| "missing 'symbol' parameter".to_string())?;
    let ids = find_symbols_by_name(sym_name, &state.ingest, &state.interner);

    let mut callers = Vec::new();
    for id in ids {
        if let Some(nav) = navigate_1hop(
            id,
            &state.ingest,
            &state.graph,
            &state.ranks,
            &state.interner,
        ) {
            callers.extend(nav.callers);
        }
    }
    Ok(json!({ "symbol": sym_name, "callers": callers }))
}

fn call_callees(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = args["symbol"]
        .as_str()
        .ok_or_else(|| "missing 'symbol' parameter".to_string())?;
    let ids = find_symbols_by_name(sym_name, &state.ingest, &state.interner);

    let mut callees = Vec::new();
    for id in ids {
        if let Some(nav) = navigate_1hop(
            id,
            &state.ingest,
            &state.graph,
            &state.ranks,
            &state.interner,
        ) {
            callees.extend(nav.callees);
        }
    }
    Ok(json!({ "symbol": sym_name, "callees": callees }))
}

fn call_impact(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = args["symbol"]
        .as_str()
        .ok_or_else(|| "missing 'symbol' parameter".to_string())?;
    let max_depth = args["max_depth"].as_u64().map(|d| d as u32);
    let ids = find_symbols_by_name(sym_name, &state.ingest, &state.interner);

    let mut impacts = Vec::new();
    for id in ids {
        let imp = transitive_impact(
            id,
            &state.ingest,
            &state.graph,
            &state.ranks,
            &state.interner,
            max_depth,
        );
        impacts.extend(imp);
    }
    Ok(json!({ "symbol": sym_name, "impact": impacts }))
}

fn call_explore(state: &McpState, args: &Value) -> Result<Value, String> {
    let top_k = args["top_k"].as_u64().map(|k| k as usize);
    let xml = serialize_xml_to_string(
        &state.ingest,
        &state.graph,
        &state.pr,
        &state.interner,
        &SerializeOptions {
            root: state.root.clone(),
            top_k,
            include_pr: true,
        },
    );
    Ok(Value::String(xml))
}

fn call_cochange(state: &McpState, args: &Value) -> Result<Value, String> {
    let file_path = args["path"]
        .as_str()
        .ok_or_else(|| "missing 'path' parameter".to_string())?;
    let since_str = args["since"].as_str();

    let empty_commits = Vec::new();
    let commits_ref = state.commits.as_ref().unwrap_or(&empty_commits);
    let anchor_epoch = commits_ref.first().map(|c| c.timestamp).unwrap_or(0);
    let scope = crate::git::history::parse_since_scope(since_str, anchor_epoch);
    let filtered = crate::git::history::filter_commits_by_scope(commits_ref, &scope);

    let coupling = crate::git::cochange::StaticCouplingMap::build(
        &state.ingest,
        &state.interner,
        Some(&state.graph),
    );
    let opts = crate::git::cochange::CochangeOptions::default();
    let partners = crate::git::cochange::mine_cochange_for_file(
        file_path,
        filtered,
        Some(&coupling),
        Some(&state.ingest),
        Some(&state.interner),
        &opts,
    );

    let partners_json: Vec<Value> = partners
        .into_iter()
        .map(|p| {
            json!({
                "file": p.file_path,
                "together": p.together,
                "deg": (p.deg * 100.0).round() / 100.0,
                "deg_rev": (p.deg_rev * 100.0).round() / 100.0,
                "recur": p.recur,
                "surprising": p.surprising,
                "dep_capable": p.dep_capable
            })
        })
        .collect();

    Ok(json!({
        "path": file_path,
        "partners": partners_json
    }))
}

fn call_edit_check(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = args["symbol"]
        .as_str()
        .ok_or_else(|| "missing 'symbol' parameter".to_string())?;

    let ids = find_symbols_by_name(sym_name, &state.ingest, &state.interner);
    let Some(&sym_id) = ids.first() else {
        return Ok(json!({
            "found": false,
            "message": format!("symbol '{}' not found", sym_name)
        }));
    };

    let sym = &state.ingest.symbols[sym_id as usize];
    let rel_path = state
        .interner
        .resolve(&state.ingest.files[sym.file_id as usize].path);
    let head_contract = crate::gate::contract::load_head_symbol_contract(
        Path::new(&state.root),
        rel_path,
        sym_name,
        sym.lang,
    )
    .ok()
    .flatten();

    let report = crate::gate::edit_check::check_symbol_contract(
        sym_id,
        head_contract.as_ref(),
        &state.ingest,
        &state.graph,
        &state.interner,
    );

    let Some(rep) = report else {
        return Ok(json!({ "found": false }));
    };

    Ok(json!({
        "symbol": rep.symbol_name,
        "file": rep.file_path,
        "status": rep.status.as_str(),
        "total_callers": rep.total_callers,
        "incompatible_callers": rep.incompatible_callers,
        "callers": rep.callers.into_iter().map(|c| {
            json!({
                "caller": c.caller_name,
                "file": c.file_path,
                "line": c.line,
                "incompatible": c.is_incompatible,
                "sites": c.sites.into_iter().map(|s| {
                    json!({ "line": s.line, "args": s.arg_count, "required": s.required_params })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>()
    }))
}

fn call_affected(state: &McpState, args: &Value) -> Result<Value, String> {
    let files_spec = args["files"]
        .as_str()
        .ok_or_else(|| "missing 'files' parameter".to_string())?;
    let specs: Vec<&str> = files_spec.split(',').map(|s| s.trim()).collect();
    let rep = crate::gate::affected::find_affected_tests(
        &specs,
        &state.ingest,
        &state.graph,
        &state.interner,
    );

    Ok(json!({
        "changed": rep.changed_spec,
        "seed_count": rep.seed_count,
        "seed_test_files": rep.seed_test_files,
        "total_tests": rep.total_tests,
        "reached_symbols": rep.reached_symbols,
        "tests": rep.tests.into_iter().map(|t| {
            json!({ "path": t.path, "hops": t.hops, "direct": t.is_direct_change })
        }).collect::<Vec<_>>()
    }))
}

fn call_quality_baseline(state: &McpState, _args: &Value) -> Result<Value, String> {
    let mut out = Vec::new();
    crate::cli::execute_quality_baseline_with(state, &mut out).map_err(|e| e.to_string())?;
    let msg = String::from_utf8(out).unwrap_or_default();
    Ok(json!({ "result": msg.trim() }))
}

fn call_quality_delta(state: &McpState, _args: &Value) -> Result<Value, String> {
    let report = crate::gate::quality_delta::evaluate_quality_delta(&state.root, state)
        .map_err(|e| e.to_string())?;

    Ok(json!({
        "baseline_source": report.baseline_source,
        "head_sha": report.head_sha,
        "gating_count": report.gating_count,
        "total_regressions": report.regressions.len(),
        "has_gating_failures": report.has_gating_failures(),
        "regressions": report.regressions.into_iter().map(|r| {
            json!({
                "kind": r.kind,
                "symbol": r.sym_name,
                "file": r.file_path,
                "line": r.line,
                "was": r.was_val,
                "now": r.now_val,
                "gating": r.is_gating,
                "new_symbol": r.is_new_symbol
            })
        }).collect::<Vec<_>>()
    }))
}

/// Handle `tools/call` method.
#[must_use]
pub fn handle_tools_call(
    id: &Value,
    params: &Value,
    state: &mut Option<McpState>,
    default_root: &str,
) -> Value {
    let tool_name = match params["name"].as_str() {
        Some(name) => name,
        None => {
            return json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32602, "message": "missing tool 'name'" }
            });
        }
    };

    if state.is_none() {
        *state = McpState::load(default_root);
    }
    let Some(current_state) = state.as_mut() else {
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32603, "message": "failed to index workspace root" }
        });
    };

    let args = &params["arguments"];
    let result = match tool_name {
        "find_symbol" => call_find_symbol(current_state, args),
        "callers" => call_callers(current_state, args),
        "callees" => call_callees(current_state, args),
        "impact" => call_impact(current_state, args),
        "explore" => call_explore(current_state, args),
        "cochange" => call_cochange(current_state, args),
        "edit_check" => call_edit_check(current_state, args),
        "affected" => call_affected(current_state, args),
        "quality_baseline" => call_quality_baseline(current_state, args),
        "quality_delta" => call_quality_delta(current_state, args),
        _ => Err(format!("unknown tool: {}", tool_name)),
    };
    match result {
        Ok(val) => {
            let text = match val {
                Value::String(s) => s,
                other => serde_json::to_string_pretty(&other).unwrap_or_default(),
            };
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [
                        {
                            "type": "text",
                            "text": text
                        }
                    ]
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

/// Dispatch a single raw JSON-RPC line. Returns `None` for notifications.
#[must_use]
pub fn dispatch_json_rpc(
    line: &str,
    state: &mut Option<McpState>,
    default_root: &str,
) -> Option<String> {
    let req: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(_) => {
            let err = json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": { "code": -32700, "message": "Parse error" }
            });
            return Some(err.to_string());
        }
    };

    let id = req.id.as_ref()?;

    let resp = match req.method.as_str() {
        "initialize" => handle_initialize(id),
        "tools/list" => handle_tools_list(id),
        "tools/call" => {
            let params = req.params.as_ref().unwrap_or(&Value::Null);
            handle_tools_call(id, params, state, default_root)
        }
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Method not found: {}", req.method) }
        }),
    };

    Some(resp.to_string())
}

/// Run MCP JSON-RPC loop over any reader and writer.
pub fn run_mcp_stream<R: BufRead, W: Write>(
    reader: R,
    writer: &mut W,
    default_root: &str,
) -> io::Result<()> {
    let mut state = None;
    for line_res in reader.lines() {
        let line = line_res?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(resp) = dispatch_json_rpc(&line, &mut state, default_root) {
            writeln!(writer, "{}", resp)?;
            writer.flush()?;
        }
    }
    Ok(())
}

/// Run stdio MCP JSON-RPC loop.
pub fn run_mcp_stdio(default_root: &str) -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    run_mcp_stream(stdin.lock(), &mut stdout, default_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_initialize_and_tools_list() {
        let init_req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let mut state = None;
        let resp = dispatch_json_rpc(init_req, &mut state, ".").expect("should reply");
        let val: Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(val["id"], 1);
        assert_eq!(val["result"]["serverInfo"]["name"], "rwire");

        let list_req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
        let list_resp = dispatch_json_rpc(list_req, &mut state, ".").expect("should reply");
        let list_val: Value = serde_json::from_str(&list_resp).unwrap();
        assert_eq!(list_val["id"], 2);
        let tools = list_val["result"]["tools"].as_array().unwrap();
        let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(tool_names.contains(&"explore"));
        assert!(tool_names.contains(&"find_symbol"));
        assert!(tool_names.contains(&"callers"));
        assert!(tool_names.contains(&"callees"));
        assert!(tool_names.contains(&"impact"));

        // Notifications have no id and should produce no response
        let notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        assert!(dispatch_json_rpc(notif, &mut state, ".").is_none());
    }
}
