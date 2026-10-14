use super::doc_tools;
use super::edit;
use super::graph_tools;
use super::retrieval;
use super::state::McpState;
use crate::ingest::LanguageRegistry;
use crate::navigate::{find_symbols_by_name, navigate_1hop, transitive_impact};
use serde_json::{Value, json};
use std::path::Path;
#[inline]
pub fn require_str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args[key]
        .as_str()
        .ok_or_else(|| format!("missing '{key}' parameter"))
}

#[inline]
pub fn opt_str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args[key].as_str()
}

pub fn call_find_symbol(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = require_str_arg(args, "symbol")?;
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

pub fn call_callers(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = require_str_arg(args, "symbol")?;
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

pub fn call_callees(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = require_str_arg(args, "symbol")?;
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

pub fn call_impact(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = require_str_arg(args, "symbol")?;
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

pub fn call_explore(state: &McpState, args: &Value) -> Result<Value, String> {
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
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let xml = String::from_utf8(out).map_err(|e| e.to_string())?;
    Ok(Value::String(xml))
}

pub fn call_cochange(state: &McpState, args: &Value) -> Result<Value, String> {
    let path = require_str_arg(args, "path")?;
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

pub fn call_edit_check(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = require_str_arg(args, "symbol")?;

    let ids = crate::navigate::find_symbols_by_name(sym_name, &state.ingest, &state.interner);
    let Some(&sym_id) = ids.first() else {
        return Ok(json!({ "error": format!("symbol '{sym_name}' not found") }));
    };

    let sym = &state.ingest.symbols[sym_id as usize];
    let rel_path = state
        .interner
        .resolve(&state.ingest.files[sym.file_id as usize].path);

    let baseline = crate::gate::contract::load_head_symbol_contract(
        Path::new(&state.root),
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

pub fn call_affected(state: &McpState, args: &Value) -> Result<Value, String> {
    let files_spec = args["files"]
        .as_str()
        .ok_or_else(|| "missing 'files' parameter".to_string())?;
    let specs: Vec<&str> = files_spec.split(',').map(str::trim).collect();
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

pub fn call_quality_baseline(state: &McpState, _args: &Value) -> Result<Value, String> {
    let mut out = Vec::new();
    crate::cli::execute_quality_baseline_with(state, &mut out).map_err(|e| e.to_string())?;
    let msg = String::from_utf8(out).unwrap_or_default();
    Ok(json!({ "result": msg.trim() }))
}

pub fn call_quality_delta(state: &McpState, _args: &Value) -> Result<Value, String> {
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

pub fn call_slice(state: &McpState, args: &Value) -> Result<Value, String> {
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

pub fn call_edit_tool(
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

    Ok(edit::perform_atomic_edit_in_root(
        &state.root,
        &state.ingest,
        &state.interner,
        op,
        symbol,
        path_hint,
        payload,
    ))
}

pub fn dispatch_read_tools(
    state: &McpState,
    tool_name: &str,
    args: &Value,
) -> Result<Value, String> {
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
            let detail = args["detail"].as_u64().map(|d| d as usize);
            let adaptive = args["adaptive"].as_bool().unwrap_or(false);
            Ok(retrieval::call_for_task_with_options(
                &state.ingest,
                &state.interner,
                &state.ranks,
                task,
                retrieval::ForTaskOptions {
                    root: &state.root,
                    graph: Some(&state.graph),
                    budget_tokens: budget,
                    detail,
                    adaptive,
                },
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
            Ok(retrieval::call_fetch_body_in_root(
                &state.root,
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
            Ok(retrieval::call_exemplar_in_root(
                &state.root,
                &state.ingest,
                &state.interner,
                &state.ranks,
                role,
                Some(&state.graph),
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
            Ok(retrieval::call_memory_recall_in_root(
                &state.root,
                &state.ingest,
                &state.interner,
                task,
                top_k,
            ))
        }
        "grep" => {
            let pattern = args["pattern"].as_str().unwrap_or("");
            let path_filter = args["path"].as_str();
            Ok(retrieval::call_grep_in_root(
                &state.root,
                &state.ingest,
                &state.interner,
                pattern,
                path_filter,
            ))
        }
        _ => Err(format!("not a read tool: {tool_name}")),
    }
}

pub fn dispatch_graph_and_doc_tools(
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
            Ok(doc_tools::call_mentions_in_root(
                &state.root,
                &state.ingest,
                &state.interner,
                sym,
            ))
        }
        "doc_drift" => Ok(doc_tools::call_doc_drift_in_root(
            &state.root,
            &state.ingest,
            &state.interner,
        )),
        "flags" => Ok(doc_tools::call_flags(&state.ingest, &state.interner)),
        "owners" => {
            let path = args["path"].as_str().or_else(|| args["file"].as_str());
            Ok(doc_tools::call_owners(
                state.commits.as_deref(),
                path,
                None,
                None,
            ))
        }
        "whereis" => {
            let sym = args["symbol"].as_str().unwrap_or("");
            let with_hist = args["with_history"].as_bool().unwrap_or(false);
            Ok(doc_tools::call_whereis(
                &state.ingest,
                &state.interner,
                &state.root,
                sym,
                with_hist,
            ))
        }
        "stray_content" => Ok(doc_tools::call_stray_content(&state.root)),
        _ => Err(format!("unknown tool: {tool_name}")),
    }
}

pub fn dispatch_tool_action(
    state: &McpState,
    tool_name: &str,
    args: &Value,
) -> Result<Value, String> {
    dispatch_read_tools(state, tool_name, args)
        .or_else(|_| dispatch_graph_and_doc_tools(state, tool_name, args))
}

fn attach_session_legend_meta(val: &mut Value, state: &McpState, args: &Value) {
    let first_turn = !state
        .legend_sent
        .swap(true, std::sync::atomic::Ordering::Relaxed);
    let force_compact = args["legend"].as_str() == Some("compact");
    let Value::Object(map) = val else {
        return;
    };
    if first_turn || force_compact {
        map.insert(
            "dictv".to_string(),
            Value::String(crate::serialize::LEGEND_DICT_VERSION.to_string()),
        );
    } else {
        map.insert(
            "about".to_string(),
            json!({ "legend": "ref", "dictv": crate::serialize::LEGEND_DICT_VERSION }),
        );
    }
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
        Ok(mut val) => {
            attach_session_legend_meta(&mut val, current_state, args);
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
