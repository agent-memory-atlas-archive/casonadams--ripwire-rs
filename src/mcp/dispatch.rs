use super::doc_tools;
use super::edit;
use super::graph_tools;
use super::retrieval;
use super::state::McpState;
use crate::ingest::LanguageRegistry;
use crate::navigate::{find_symbols_by_name, navigate_1hop, transitive_impact};
use crate::serialize::JsonValueExt;
use serde_json::{Value, json};
#[inline]
pub fn require_str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.req_str(key)
}

#[inline]
pub fn opt_str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.opt_str(key)
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
                .filter_map(|(cid, _)| state.ingest.sym(cid))
                .map(|cs| &state.interner[cs.name])
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
                .filter_map(|(cid, _)| state.ingest.sym(cid))
                .map(|cs| &state.interner[cs.name])
                .collect::<Vec<_>>();
            json!({ "symbol_id": id, "callees": callees })
        })
        .collect();

    Ok(json!({ "symbol": sym_name, "definitions": callees_json }))
}

pub fn call_impact(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym_name = args.req_str("symbol")?;
    let max_depth = args.u64_or("max_depth", 8) as u32;
    use crate::domain::CorpusView;
    let ids = find_symbols_by_name(sym_name, &state.ingest, &state.interner);
    let importers = (&state.ingest, &state.interner).resolve_symbol_importers(&ids);
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

    Ok(json!({ "symbol": sym_name, "importers": importers, "impact": blast_radius }))
}

pub fn call_explore(state: &McpState, args: &Value) -> Result<Value, String> {
    let top_k = args.opt_usize("top_k");
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

fn resolve_state_commits(state: &McpState, months: u32) -> Vec<crate::git::history::CommitInfo> {
    if let Some(commits) = &state.commits {
        return commits.clone();
    }
    let cutoff = crate::git::history::resolve_cutoff_timestamp(None, months);
    crate::git::history::walk_repository_history_scoped(&state.root, cutoff).unwrap_or_default()
}

pub fn call_cochange(state: &McpState, args: &Value) -> Result<Value, String> {
    let path = require_str_arg(args, "path")?;
    let commits = resolve_state_commits(state, 18);
    let coupling = crate::git::cochange::StaticCouplingMap::build(
        &state.ingest,
        &state.interner,
        Some(&state.graph),
    );
    let opts = crate::git::cochange::CochangeOptions::default();
    let partners = crate::git::cochange::mine_cochange_for_file(
        path,
        &commits,
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

    let sym = &state.ingest[sym_id];
    let rel_path = state.ingest.sym_file_path(sym, &state.interner);
    let baseline =
        crate::gate::contract::load_head_symbol_contract(&state.root, rel_path, sym_name, sym.lang)
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
    let files_spec = args.req_str("files")?;
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

pub fn call_quality_baseline(state: &McpState, args: &Value) -> Result<Value, String> {
    let mut out = Vec::new();
    let allow_dirty = args.bool_or("allow_dirty", false);
    crate::cli::execute_quality_baseline_with(state, allow_dirty, &mut out)
        .map_err(|e| e.to_string())?;
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
    let target = args.req_str_any(&["target", "symbol"])?;
    let flow_str = args.str_or("flow", "fwd");
    let flow_dir = flow_str.parse().unwrap_or_default();
    let registry = LanguageRegistry::new().map_err(|e| e.to_string())?;
    use crate::domain::CorpusView;
    use crate::navigate::SymbolLookup;
    let sym_query = target.split(':').next().unwrap_or(target);
    let custom_bytes = state
        .find_first_symbol(sym_query)
        .and_then(|s| state.read_sym_bytes(&state.root, s).ok());
    let xml = crate::slice::execute_slice(
        &state.ingest,
        &registry,
        &state.interner,
        target,
        flow_dir,
        8,
        custom_bytes.as_deref(),
    )?;
    Ok(json!({ "slice": xml }))
}

pub fn call_edit_tool(
    state: &McpState,
    op: edit::EditOp,
    args: &Value,
    is_replace: bool,
) -> Result<Value, String> {
    let symbol = args.req_str("symbol")?;
    let payload = args.req_str(if is_replace { "new_body" } else { "text" })?;
    let path_hint = args.opt_str_any(&["path", "file"]);
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

pub fn call_for_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let task = args.str_or("task", "");
    let budget = args.opt_usize("budget_tokens");
    let detail = args.opt_usize("detail");
    let adaptive = args.bool_or("adaptive", false);
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

pub fn call_fetch_body_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym = args.str_any(&["symbol", "handle"], "");
    let path_hint = args.opt_str_any(&["path", "file"]);
    let sl = args.opt_usize("start_line");
    let el = args.opt_usize("end_line");
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

pub fn call_exemplar_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let role = args.str_any(&["role", "kind"], "fn");
    Ok(retrieval::call_exemplar_in_root(
        &state.root,
        &state.ingest,
        &state.interner,
        &state.ranks,
        role,
        Some(&state.graph),
    ))
}

pub fn call_lego_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let iface = args.str_any(&["interface", "symbol"], "");
    Ok(retrieval::call_lego(&state.ingest, &state.interner, iface))
}

pub fn call_memory_recall_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let task = args.str_or("task", "");
    let top_k = args.opt_usize("top_k");
    Ok(retrieval::call_memory_recall_in_root(
        &state.root,
        &state.ingest,
        &state.interner,
        task,
        top_k,
    ))
}

pub fn call_grep_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let pattern = args.str_or("pattern", "");
    let path_filter = args.opt_str("path");
    Ok(retrieval::call_grep_in_root(
        &state.root,
        &state.ingest,
        &state.interner,
        pattern,
        path_filter,
    ))
}

pub fn call_connect_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let symbols = args.str_vec("symbols");
    Ok(graph_tools::call_connect(
        &state.ingest,
        &state.graph,
        &state.interner,
        &symbols,
    ))
}

pub fn call_path_between_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let from = args.str_or("from", "");
    let to = args.str_or("to", "");
    Ok(graph_tools::call_path_between(
        &state.ingest,
        &state.graph,
        &state.interner,
        from,
        to,
    ))
}

pub fn call_uses_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym = args.str_or("symbol", "");
    Ok(graph_tools::call_uses(&state.ingest, &state.interner, sym))
}

pub fn call_analyze_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let top_k = args.opt_usize("top_k");
    let rank_by = args.opt_str("rank_by");
    Ok(graph_tools::call_analyze(
        &state.ingest,
        &state.graph,
        &state.interner,
        &state.ranks,
        top_k,
        rank_by,
    ))
}

pub fn call_rank_by_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let rank_by = args.str_or("rank_by", "pagerank");
    let top_k = args.opt_usize("top_k");
    Ok(graph_tools::call_analyze(
        &state.ingest,
        &state.graph,
        &state.interner,
        &state.ranks,
        top_k,
        Some(rank_by),
    ))
}

pub fn call_situational_awareness_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let changed = args.str_vec("files");
    let commits = resolve_state_commits(state, 18);
    Ok(graph_tools::call_situational_awareness(
        &state.ingest,
        &state.graph,
        &state.interner,
        &changed,
        Some(&commits),
    ))
}

pub fn call_from_trace_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let trace = args.str_or("trace", "");
    Ok(graph_tools::call_from_trace(
        &state.ingest,
        &state.interner,
        trace,
    ))
}

pub fn call_mentions_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym = args.str_or("symbol", "");
    Ok(doc_tools::call_mentions_in_root(
        &state.root,
        &state.ingest,
        &state.interner,
        sym,
    ))
}

pub fn call_doc_drift_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let with_hist = args.bool_or("with_history", false);
    Ok(doc_tools::call_doc_drift_with_history(
        &state.root,
        &state.ingest,
        &state.interner,
        with_hist,
    ))
}

pub fn call_flags_tool(state: &McpState, _args: &Value) -> Result<Value, String> {
    Ok(doc_tools::call_flags_in_root(
        &state.root,
        &state.ingest,
        &state.interner,
    ))
}

pub fn call_owners_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let path = args.opt_str_any(&["path", "file"]);
    let commits = resolve_state_commits(state, 12);
    Ok(doc_tools::call_owners(Some(&commits), path, None, None))
}

pub fn call_whereis_tool(state: &McpState, args: &Value) -> Result<Value, String> {
    let sym = args.str_or("symbol", "");
    let with_hist = args.bool_or("with_history", false);
    Ok(doc_tools::call_whereis(
        &state.ingest,
        &state.interner,
        &state.root,
        sym,
        with_hist,
    ))
}
pub fn call_stray_content_tool(state: &McpState, _args: &Value) -> Result<Value, String> {
    Ok(doc_tools::call_stray_content(&state.root))
}

pub fn call_batch(state: &McpState, args: &Value) -> Result<Value, String> {
    let queries = args["queries"].as_array().cloned().unwrap_or_default();
    let mut results = Vec::new();
    for q in queries.into_iter().take(16) {
        let q_name = q["name"].as_str().unwrap_or("");
        let q_args = &q["arguments"];
        let res = dispatch_tool_action(state, q_name, q_args);
        results.push(match res {
            Ok(v) => json!({ "tool": q_name, "result": v }),
            Err(e) => json!({ "tool": q_name, "error": e }),
        });
    }
    Ok(Value::Array(results))
}

/// Trait encapsulating an MCP tool handler and its schema metadata.
pub trait McpToolHandler: Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[]
    }
    fn required(&self) -> &'static [&'static str] {
        &[]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String>;

    fn matches_name(&self, tool_name: &str) -> bool {
        self.name() == tool_name
    }

    fn to_schema_json(&self) -> Value {
        super::schema::build_tool_schema_json(
            self.name(),
            self.description(),
            self.params(),
            self.required(),
        )
    }
}

pub fn dispatch_tool_action(
    state: &McpState,
    tool_name: &str,
    args: &Value,
) -> Result<Value, String> {
    for handler in super::schema::MCP_TOOL_HANDLERS {
        if handler.matches_name(tool_name) {
            return handler.execute(state, args);
        }
    }
    Err(format!("unknown tool: {tool_name}"))
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
    let Some(tool_name) = params["name"].as_str() else {
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32602, "message": "missing tool 'name'" }
        });
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
    match dispatch_tool_action(current_state, tool_name, args) {
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
