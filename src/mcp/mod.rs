pub mod doc_tools;
pub mod edit;
pub mod graph_tools;
pub mod retrieval;
pub mod schema;

use crate::crawler::{crawl_directory, CrawlerOptions};
use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::ingest::{ingest_crawled_files, LanguageRegistry};
use crate::navigate::{find_symbols_by_name, navigate_1hop, transitive_impact};
use crate::rank::{PageRankConfig, PageRankResult};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

/// Server state caching the indexed workspace facts and call graph.
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
        let registry = LanguageRegistry::new().ok()?;
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

fn call_find_symbol(state: &McpState, args: &Value) -> Result<Value, String> {
    let Some(sym_name) = args["symbol"].as_str() else {
        return Err("missing 'symbol' parameter".to_string());
    };
    let ids = find_symbols_by_name(sym_name, &state.ingest, &state.interner);
    let mut matches = Vec::new();
    for id in ids {
        if let Some(nav) = navigate_1hop(
            id,
            &state.ingest,
            &state.graph,
            &state.ranks,
            &state.interner,
        ) {
            matches.push(json!({
                "target": nav.target,
                "callers": nav.callers,
                "callees": nav.callees,
            }));
        }
    }
    Ok(json!({ "symbol": sym_name, "matches": matches }))
}

fn call_callers(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = args["symbol"]
        .as_str()
        .ok_or_else(|| "missing 'symbol' parameter".to_string())?;
    let ids = find_symbols_by_name(sym_name, &state.ingest, &state.interner);

    let callers_json: Vec<Value> = ids
        .into_iter()
        .map(|id| {
            let callers = state
                .graph
                .callers(id)
                .filter_map(|(cid, _)| state.ingest.symbols.get(cid as usize))
                .map(|cs| state.interner.resolve(&cs.name))
                .collect::<Vec<_>>();
            json!({ "symbol_id": id, "callers": callers })
        })
        .collect();

    Ok(json!({ "symbol": sym_name, "definitions": callers_json }))
}

fn call_callees(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = args["symbol"]
        .as_str()
        .ok_or_else(|| "missing 'symbol' parameter".to_string())?;
    let ids = find_symbols_by_name(sym_name, &state.ingest, &state.interner);

    let callees_json: Vec<Value> = ids
        .into_iter()
        .map(|id| {
            let callees = state
                .graph
                .callees(id)
                .filter_map(|(cid, _)| state.ingest.symbols.get(cid as usize))
                .map(|cs| state.interner.resolve(&cs.name))
                .collect::<Vec<_>>();
            json!({ "symbol_id": id, "callees": callees })
        })
        .collect();

    Ok(json!({ "symbol": sym_name, "definitions": callees_json }))
}

fn call_impact(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = args["symbol"]
        .as_str()
        .ok_or_else(|| "missing 'symbol' parameter".to_string())?;
    let max_depth = args["max_depth"].as_u64().unwrap_or(8) as u32;

    let ids = find_symbols_by_name(sym_name, &state.ingest, &state.interner);
    let blast_radius: Vec<Value> = ids
        .into_iter()
        .map(|id| {
            let impact_nodes = transitive_impact(
                id,
                &state.ingest,
                &state.graph,
                &state.ranks,
                &state.interner,
                Some(max_depth),
            );
            json!({
                "symbol_id": id,
                "reachable_count": impact_nodes.len(),
                "nodes": impact_nodes
            })
        })
        .collect();

    Ok(json!({ "symbol": sym_name, "impact": blast_radius }))
}

fn call_explore(state: &McpState, args: &Value) -> Result<Value, String> {
    let top_k = args["top_k"].as_u64().map(|k| k as usize);
    let mut out = Vec::new();
    crate::serialize::serialize_xml(
        &state.ingest,
        &state.graph,
        &state.pr,
        &state.interner,
        &mut out,
        &crate::serialize::SerializeOptions {
            root: state.root.clone(),
            top_k,
            include_pr: true,
        },
    )
    .map_err(|e| e.to_string())?;
    let xml = String::from_utf8(out).map_err(|e| e.to_string())?;
    Ok(Value::String(xml))
}

fn call_cochange(state: &McpState, args: &Value) -> Result<Value, String> {
    let path = args["path"]
        .as_str()
        .ok_or_else(|| "missing 'path' parameter".to_string())?;
    let commits_empty = Vec::new();
    let commits_slice = state.commits.as_deref().unwrap_or(&commits_empty);
    let coupling = crate::git::cochange::StaticCouplingMap::build(
        &state.ingest,
        &state.interner,
        Some(&state.graph),
    );
    let opts = crate::git::cochange::CochangeOptions::default();
    let partners = crate::git::cochange::mine_cochange_for_file(
        path,
        commits_slice,
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
                "deg": p.deg,
                "surprising": p.surprising,
                "dep_capable": p.dep_capable
            })
        })
        .collect();

    Ok(json!({
        "path": path,
        "target": path,
        "partners_count": partners_json.len(),
        "partners": partners_json
    }))
}

fn call_edit_check(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = args["symbol"]
        .as_str()
        .ok_or_else(|| "missing 'symbol' parameter".to_string())?;

    let ids = crate::navigate::find_symbols_by_name(sym_name, &state.ingest, &state.interner);
    let Some(&sym_id) = ids.first() else {
        return Ok(json!({ "error": format!("symbol '{sym_name}' not found") }));
    };

    let sym = &state.ingest.symbols[sym_id as usize];
    let rel_path = state
        .interner
        .resolve(&state.ingest.files[sym.file_id as usize].path);

    let baseline = crate::gate::contract::load_head_symbol_contract(
        std::path::Path::new(&state.root),
        rel_path,
        sym_name,
        sym.lang,
    )
    .ok()
    .flatten();

    let rep = crate::gate::edit_check::check_symbol_contract(
        sym_id,
        baseline.as_ref(),
        &state.ingest,
        &state.graph,
        &state.interner,
    )
    .ok_or_else(|| "failed to check symbol contract".to_string())?;
    Ok(json!({
        "symbol": sym_name,
        "status": rep.status.to_string(),
        "total_callers": rep.total_callers,
        "incompatible_callers": rep.incompatible_callers,
        "callers_count": rep.total_callers,
        "incompatible_count": rep.incompatible_callers,
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

fn call_slice(state: &McpState, args: &Value) -> Result<Value, String> {
    let target = args["target"]
        .as_str()
        .or_else(|| args["symbol"].as_str())
        .ok_or_else(|| "missing 'target' or 'symbol' parameter".to_string())?;
    let flow_str = args["flow"].as_str().unwrap_or("fwd");
    let flow_dir = crate::slice::SliceFlowDir::from_str_opt(flow_str)
        .unwrap_or(crate::slice::SliceFlowDir::Fwd);

    let registry = LanguageRegistry::new().map_err(|e| e.to_string())?;
    let xml = crate::slice::execute_slice(
        &state.ingest,
        &registry,
        &state.interner,
        target,
        flow_dir,
        8,
        None,
    )?;
    Ok(json!({ "slice": xml }))
}

fn call_edit_tool(
    state: &McpState,
    op: edit::EditOp,
    args: &Value,
    is_replace: bool,
) -> Result<Value, String> {
    let symbol = args["symbol"]
        .as_str()
        .ok_or_else(|| "missing 'symbol' parameter".to_string())?;
    let payload = if is_replace {
        args["new_body"]
            .as_str()
            .ok_or_else(|| "missing 'new_body' parameter".to_string())?
    } else {
        args["text"]
            .as_str()
            .ok_or_else(|| "missing 'text' parameter".to_string())?
    };
    let path_hint = args["path"].as_str().or_else(|| args["file"].as_str());

    Ok(edit::perform_atomic_edit(
        &state.ingest,
        &state.interner,
        op,
        symbol,
        path_hint,
        payload,
    ))
}

fn dispatch_read_tools(state: &McpState, tool_name: &str, args: &Value) -> Result<Value, String> {
    match tool_name {
        "find_symbol" => call_find_symbol(state, args),
        "callers" | "find_referencing_symbols" => call_callers(state, args),
        "callees" => call_callees(state, args),
        "impact" => call_impact(state, args),
        "explore" => call_explore(state, args),
        "cochange" => call_cochange(state, args),
        "edit_check" => call_edit_check(state, args),
        "affected" => call_affected(state, args),
        "quality_baseline" => call_quality_baseline(state, args),
        "quality_delta" => call_quality_delta(state, args),
        "slice" => call_slice(state, args),
        "replace_symbol_body" => call_edit_tool(state, edit::EditOp::ReplaceBody, args, true),
        "insert_before_symbol" => call_edit_tool(state, edit::EditOp::InsertBefore, args, false),
        "insert_after_symbol" => call_edit_tool(state, edit::EditOp::InsertAfter, args, false),
        "for" => {
            let task = args["task"].as_str().unwrap_or("");
            let budget = args["budget_tokens"].as_u64().map(|b| b as usize);
            Ok(retrieval::call_for_task(
                &state.ingest,
                &state.interner,
                &state.ranks,
                task,
                budget,
            ))
        }
        "fetch_body" => {
            let sym = args["symbol"]
                .as_str()
                .or_else(|| args["handle"].as_str())
                .unwrap_or("");
            let path_hint = args["path"].as_str().or_else(|| args["file"].as_str());
            let sl = args["start_line"].as_u64().map(|l| l as usize);
            let el = args["end_line"].as_u64().map(|l| l as usize);
            Ok(retrieval::call_fetch_body(
                &state.ingest,
                &state.interner,
                sym,
                path_hint,
                sl,
                el,
            ))
        }
        "exemplar" => {
            let role = args["role"]
                .as_str()
                .or_else(|| args["kind"].as_str())
                .unwrap_or("fn");
            Ok(retrieval::call_exemplar(
                &state.ingest,
                &state.interner,
                &state.ranks,
                role,
            ))
        }
        "lego" => {
            let iface = args["interface"]
                .as_str()
                .or_else(|| args["symbol"].as_str())
                .unwrap_or("");
            Ok(retrieval::call_lego(&state.ingest, &state.interner, iface))
        }
        "memory_recall" => {
            let task = args["task"].as_str().unwrap_or("");
            let top_k = args["top_k"].as_u64().map(|k| k as usize);
            Ok(retrieval::call_memory_recall(
                &state.ingest,
                &state.interner,
                task,
                top_k,
            ))
        }
        "grep" => {
            let pattern = args["pattern"].as_str().unwrap_or("");
            let path_filter = args["path"].as_str();
            Ok(retrieval::call_grep(
                &state.ingest,
                &state.interner,
                pattern,
                path_filter,
            ))
        }
        _ => Err(format!("not a read tool: {tool_name}")),
    }
}

fn dispatch_graph_and_doc_tools(
    state: &McpState,
    tool_name: &str,
    args: &Value,
) -> Result<Value, String> {
    match tool_name {
        "connect" => {
            let symbols: Vec<String> = args["symbols"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            Ok(graph_tools::call_connect(
                &state.ingest,
                &state.graph,
                &state.interner,
                &symbols,
            ))
        }
        "path_between" => {
            let from = args["from"].as_str().unwrap_or("");
            let to = args["to"].as_str().unwrap_or("");
            Ok(graph_tools::call_path_between(
                &state.ingest,
                &state.graph,
                &state.interner,
                from,
                to,
            ))
        }
        "uses" => {
            let sym = args["symbol"].as_str().unwrap_or("");
            Ok(graph_tools::call_uses(&state.ingest, &state.interner, sym))
        }
        "analyze" => {
            let top_k = args["top_k"].as_u64().map(|k| k as usize);
            let rank_by = args["rank_by"].as_str();
            Ok(graph_tools::call_analyze(
                &state.ingest,
                &state.graph,
                &state.interner,
                &state.ranks,
                top_k,
                rank_by,
            ))
        }
        "rank_by" => {
            let rank_by = args["rank_by"].as_str().unwrap_or("pagerank");
            let top_k = args["top_k"].as_u64().map(|k| k as usize);
            Ok(graph_tools::call_analyze(
                &state.ingest,
                &state.graph,
                &state.interner,
                &state.ranks,
                top_k,
                Some(rank_by),
            ))
        }
        "situational_awareness" => {
            let changed: Vec<String> = args["files"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            Ok(graph_tools::call_situational_awareness(
                &state.ingest,
                &state.graph,
                &state.interner,
                &changed,
                state.commits.as_deref(),
            ))
        }
        "from_trace" => {
            let trace = args["trace"].as_str().unwrap_or("");
            Ok(graph_tools::call_from_trace(
                &state.ingest,
                &state.interner,
                trace,
            ))
        }
        "mentions" => {
            let sym = args["symbol"].as_str().unwrap_or("");
            Ok(doc_tools::call_mentions(
                &state.ingest,
                &state.interner,
                sym,
            ))
        }
        "doc_drift" => Ok(doc_tools::call_doc_drift(&state.ingest, &state.interner)),
        "flags" => Ok(doc_tools::call_flags(&state.ingest, &state.interner)),
        "owners" => {
            let path = args["path"].as_str().or_else(|| args["file"].as_str());
            Ok(doc_tools::call_owners(state.commits.as_deref(), path))
        }
        "whereis" => {
            let sym = args["symbol"].as_str().unwrap_or("");
            Ok(doc_tools::call_whereis(&state.ingest, &state.interner, sym))
        }
        "stray_content" => Ok(doc_tools::call_stray_content(state.commits.as_deref())),
        _ => Err(format!("unknown tool: {tool_name}")),
    }
}

fn dispatch_tool_action(state: &McpState, tool_name: &str, args: &Value) -> Result<Value, String> {
    dispatch_read_tools(state, tool_name, args)
        .or_else(|_| dispatch_graph_and_doc_tools(state, tool_name, args))
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

    if tool_name == "batch" {
        let queries = args["queries"].as_array().cloned().unwrap_or_default();
        let mut results = Vec::new();
        for q in queries.into_iter().take(16) {
            let q_name = q["name"].as_str().unwrap_or("");
            let q_args = &q["arguments"];
            let res = dispatch_tool_action(current_state, q_name, q_args);
            results.push(match res {
                Ok(v) => json!({ "tool": q_name, "result": v }),
                Err(e) => json!({ "tool": q_name, "error": e }),
            });
        }
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&results).unwrap_or_default() }]
            }
        });
    }

    let result = dispatch_tool_action(current_state, tool_name, args);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_initialize_and_tools_list() {
        let id = json!(1);
        let init_resp = handle_initialize(&id);
        assert_eq!(init_resp["result"]["serverInfo"]["name"], "rwire");

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
