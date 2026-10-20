use super::dispatch::{
    McpToolHandler, call_affected, call_batch, call_callees, call_callers, call_cochange,
    call_edit_check, call_edit_tool, call_explore, call_find_symbol, call_for_tool, call_impact,
    call_quality_baseline, call_quality_delta, call_slice, resolve_state_commits,
};
use super::edit::EditOp;
use super::state::McpState;
use super::{doc_tools, graph_tools, retrieval};
use crate::serialize::JsonValueExt;
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

/// Declarative descriptor implementing `McpToolHandler`.
pub struct McpToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub params: &'static [(&'static str, &'static str, &'static str)],
    pub required: &'static [&'static str],
    pub handler: fn(&McpState, &Value) -> Result<Value, String>,
}

impl McpToolHandler for McpToolSpec {
    fn name(&self) -> &'static str {
        self.name
    }
    fn description(&self) -> &'static str {
        self.description
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        self.params
    }
    fn required(&self) -> &'static [&'static str] {
        self.required
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        (self.handler)(state, args)
    }
}

macro_rules! mcp_tool {
    ($name:expr, $desc:expr, $params:expr, $req:expr, $handler:expr) => {
        &McpToolSpec {
            name: $name,
            description: $desc,
            params: $params,
            required: $req,
            handler: $handler,
        }
    };
}

const SYM_PARAM: &[(&str, &str, &str)] = &[("symbol", "string", "Symbol name")];
const SYM_LOOKUP_PARAM: &[(&str, &str, &str)] = &[("symbol", "string", "Symbol name to look up")];
const EDIT_INS_PARAMS: &[(&str, &str, &str)] = &[
    ("symbol", "string", "Symbol name"),
    ("text", "string", "Text to insert"),
    ("path", "string", "Optional file path hint"),
];

pub static MCP_TOOL_HANDLERS: &[&dyn McpToolHandler] = &[
    mcp_tool!(
        "affected",
        "WHICH TESTS TO RUN for a change — test files that transitively reach the changed files/symbols",
        &[(
            "files",
            "string",
            "Changed files or symbols, comma-separated"
        )],
        &["files"],
        call_affected
    ),
    mcp_tool!(
        "analyze",
        "Architecture map for a directory: top symbols, signatures, and direct callees",
        &[
            ("top_k", "integer", "Maximum number of symbols"),
            (
                "rank_by",
                "string",
                "Signal to rank by: pagerank, degree, ccx, churn"
            ),
        ],
        &[],
        |s, a| {
            let mut val = graph_tools::call_analyze(
                &s.ingest,
                &s.graph,
                &s.interner,
                &s.ranks,
                a.opt_usize("top_k"),
                a.opt_str("rank_by"),
            );
            if !s.no_stable {
                val["order"] = serde_json::json!("order=stable");
            }
            Ok(val)
        }
    ),
    mcp_tool!(
        "batch",
        "ONE-TURN CONTEXT SWEEP: answer up to 16 heterogeneous READ sub-queries in a single call",
        &[("queries", "array", "Array of tool call objects")],
        &["queries"],
        call_batch
    ),
    mcp_tool!(
        "cochange",
        "Files that historically change together with this file (Fowler's Shotgun Surgery logical coupling)",
        &[
            (
                "path",
                "string",
                "Target file path to find co-change partners for"
            ),
            (
                "since",
                "string",
                "Time window or revision (e.g. '18 months ago')"
            ),
        ],
        &["path"],
        call_cochange
    ),
    mcp_tool!(
        "connect",
        "When a task touches 2..16 named symbols, returns the minimal subgraph relating them",
        &[("symbols", "string[]", "List of symbol names")],
        &["symbols"],
        |s, a| {
            Ok(graph_tools::call_connect(
                &s.ingest,
                &s.graph,
                &s.interner,
                &a.str_vec("symbols"),
            ))
        }
    ),
    mcp_tool!(
        "doc_drift",
        "WHICH OF THIS REPO'S DOC CLAIMS ARE NOW FALSE: verifies checkable anchors in markdown against live index",
        &[],
        &[],
        |s, a| {
            Ok(doc_tools::call_doc_drift_with_history(
                &s.root,
                &s.ingest,
                &s.interner,
                a.bool_or("with_history", false),
            ))
        }
    ),
    mcp_tool!(
        "edit_check",
        "After an edit: did SYM's contract change, and which callers no longer fit?",
        &[("symbol", "string", "Symbol name to verify")],
        &["symbol"],
        call_edit_check
    ),
    mcp_tool!(
        "exemplar",
        "BEFORE writing a function/method/class/struct, get repo's single best-in-class instance to imitate",
        &[(
            "role",
            "string",
            "Kind of symbol: fn, method, class, struct, interface"
        )],
        &[],
        |s, a| {
            Ok(retrieval::call_exemplar_in_root(
                &s.root,
                &s.ingest,
                &s.interner,
                &s.ranks,
                a.str_any(&["role", "kind"], "fn"),
                Some(&s.graph),
            ))
        }
    ),
    mcp_tool!(
        "explore",
        "ONE-call task orientation: ranked symbols, files, and architecture map",
        &[
            ("path", "string", "Repository path to explore"),
            ("top_k", "integer", "Maximum number of symbols to include"),
        ],
        &[],
        call_explore
    ),
    mcp_tool!(
        "fetch_body",
        "Full (or partial-range) source of a symbol's definition",
        &[
            ("symbol", "string", "Symbol name"),
            ("path", "string", "Optional file path hint"),
            ("start_line", "integer", "Optional 1-based start line"),
            ("end_line", "integer", "Optional 1-based end line"),
        ],
        &["symbol"],
        |s, a| {
            Ok(retrieval::call_fetch_body_in_root(
                &s.root,
                &s.ingest,
                &s.interner,
                a.str_any(&["symbol", "handle"], ""),
                a.opt_str_any(&["path", "file"]),
                a.opt_usize("start_line"),
                a.opt_usize("end_line"),
            ))
        }
    ),
    mcp_tool!(
        "find_referencing_symbols",
        "Direct (1-hop) callers of a symbol",
        SYM_LOOKUP_PARAM,
        &["symbol"],
        call_callers
    ),
    mcp_tool!(
        "find_symbol",
        "A symbol's 1-hop neighborhood: the symbol plus direct callers and callees",
        SYM_LOOKUP_PARAM,
        &["symbol"],
        call_find_symbol
    ),
    mcp_tool!(
        "flags",
        "WHAT IS BUILT BUT DARK: harvests ifndef header gates, CMake options, env reads",
        &[],
        &[],
        |s, _| Ok(doc_tools::call_flags_in_root(
            &s.root,
            &s.ingest,
            &s.interner
        ))
    ),
    mcp_tool!(
        "for",
        "Task-lens ranked signatures-only inventory of building blocks most relevant to a task",
        &[
            ("task", "string", "Task description in plain words"),
            ("budget_tokens", "integer", "Maximum number of symbols"),
        ],
        &["task"],
        call_for_tool
    ),
    mcp_tool!(
        "from_trace",
        "Paste a stack trace / compiler error and get it mapped onto indexed symbols innermost-first",
        &[("trace", "string", "Stack trace text")],
        &["trace"],
        |s, a| Ok(graph_tools::call_from_trace(
            &s.ingest,
            &s.interner,
            a.str_or("trace", "")
        ))
    ),
    mcp_tool!(
        "grep",
        "Literal search; each hit annotated with its enclosing symbol",
        &[
            ("pattern", "string", "Literal pattern to search"),
            ("path", "string", "Optional path filter"),
        ],
        &["pattern"],
        |s, a| {
            Ok(retrieval::call_grep_in_root(
                &s.root,
                &s.ingest,
                &s.interner,
                a.str_or("pattern", ""),
                a.opt_str("path"),
            ))
        }
    ),
    mcp_tool!(
        "impact",
        "Transitive blast radius of a symbol via calls, ranked by PageRank",
        &[
            (
                "symbol",
                "string",
                "Symbol name to analyze blast radius for"
            ),
            ("max_depth", "integer", "Maximum hop depth"),
        ],
        &["symbol"],
        call_impact
    ),
    mcp_tool!(
        "insert_after_symbol",
        "Insert text immediately AFTER a symbol's definition (atomic write-rename)",
        EDIT_INS_PARAMS,
        &["symbol", "text"],
        |s, a| call_edit_tool(s, EditOp::InsertAfter, a, false)
    ),
    mcp_tool!(
        "insert_before_symbol",
        "Insert text immediately BEFORE a symbol's definition (atomic write-rename)",
        EDIT_INS_PARAMS,
        &["symbol", "text"],
        |s, a| call_edit_tool(s, EditOp::InsertBefore, a, false)
    ),
    mcp_tool!(
        "lego",
        "Interface-to-impls view: method contract plus EVERY implementor",
        &[("interface", "string", "Interface, trait or base class name")],
        &["interface"],
        |s, a| {
            let mut val = retrieval::call_lego(
                &s.ingest,
                &s.interner,
                a.str_any(&["interface", "symbol"], ""),
            );
            val.attach_gauges(&s.graph);
            Ok(val)
        }
    ),
    mcp_tool!(
        "memory_recall",
        "Most relevant doc sections for a task, BM25-ranked under a token ceiling",
        &[
            ("task", "string", "Task description"),
            (
                "top_k",
                "integer",
                "Maximum number of documents (default 8)"
            ),
            (
                "budget_tokens",
                "integer",
                "Body token ceiling (default 8000)"
            ),
        ],
        &["task"],
        |s, a| {
            Ok(retrieval::call_memory_recall_in_root(
                &s.root,
                a.str_or("task", ""),
                a.opt_usize("top_k"),
                a.opt_usize("budget_tokens"),
            ))
        }
    ),
    mcp_tool!(
        "mentions",
        "Docs that name a code symbol in a backtick",
        SYM_PARAM,
        &["symbol"],
        |s, a| {
            Ok(doc_tools::call_mentions_in_root(
                &s.root,
                &s.ingest,
                &s.interner,
                a.str_or("symbol", ""),
            ))
        }
    ),
    mcp_tool!(
        "owners",
        "Bus-factor: recency-weighted author ownership per file",
        &[("path", "string", "Optional file path filter")],
        &[],
        |s, a| {
            let commits = resolve_state_commits(s, 12);
            Ok(doc_tools::call_owners(
                Some(&commits),
                a.opt_str_any(&["path", "file"]),
                None,
                None,
            ))
        }
    ),
    mcp_tool!(
        "path_between",
        "Does A REACH B, and HOW? Shortest directed call path between two symbols",
        &[
            ("from", "string", "Source symbol name"),
            ("to", "string", "Target symbol name"),
        ],
        &["from", "to"],
        |s, a| {
            Ok(graph_tools::call_path_between(
                &s.ingest,
                &s.graph,
                &s.interner,
                a.str_or("from", ""),
                a.str_or("to", ""),
            ))
        }
    ),
    mcp_tool!(
        "quality_baseline",
        "PIN the quality floor: writes .ripwire-rs_quality_baseline stamped with git HEAD sha",
        &[(
            "allow_dirty",
            "boolean",
            "Pin even when the dirty tree already holds gating findings against HEAD; the count is stamped as absorbed",
        )],
        &[],
        call_quality_baseline
    ),
    mcp_tool!(
        "quality_delta",
        "Your PR self-check: reports ONLY what your working tree made WORSE vs baseline",
        &[],
        &[],
        call_quality_delta
    ),
    mcp_tool!(
        "rank_by",
        "Architecture map ranked by a signal: pagerank, degree, ccx, churn",
        &[
            ("rank_by", "string", "pagerank, degree, ccx, churn"),
            ("top_k", "integer", "Maximum symbols to include"),
        ],
        &["rank_by"],
        |s, a| {
            Ok(graph_tools::call_analyze(
                &s.ingest,
                &s.graph,
                &s.interner,
                &s.ranks,
                a.opt_usize("top_k"),
                Some(a.str_or("rank_by", "pagerank")),
            ))
        }
    ),
    mcp_tool!(
        "replace_symbol_body",
        "Replace a symbol's ENTIRE definition with new_body (atomic write-rename)",
        &[
            ("symbol", "string", "Symbol name"),
            ("new_body", "string", "Replacement definition"),
            ("path", "string", "Optional file path hint"),
        ],
        &["symbol", "new_body"],
        |s, a| call_edit_tool(s, EditOp::ReplaceBody, a, true)
    ),
    mcp_tool!(
        "situational_awareness",
        "The 5 things to know about a diff: blast_radius, tests_to_run, forgotten co-change files, hotspots",
        &[("files", "string[]", "Changed file paths")],
        &["files"],
        |s, a| {
            let commits = resolve_state_commits(s, 18);
            Ok(graph_tools::call_situational_awareness(
                &s.ingest,
                &s.graph,
                &s.interner,
                &a.str_vec("files"),
                Some(&commits),
            ))
        }
    ),
    mcp_tool!(
        "slice",
        "ARISE intra-procedural def-use variable dataflow slice inside a function",
        &[
            ("target", "string", "sym:var or bare sym to list locals"),
            ("flow", "string", "fwd, back, or both"),
        ],
        &["target"],
        call_slice
    ),
    mcp_tool!(
        "stray_content",
        "Lines authored on divergent branches that HEAD does not have",
        &[],
        &[],
        |s, _| Ok(doc_tools::call_stray_content(&s.root))
    ),
    mcp_tool!(
        "uses",
        "All statically resolvable use-sites of a symbol: call, read, write, import, extends",
        SYM_PARAM,
        &["symbol"],
        |s, a| Ok(graph_tools::call_uses(
            &s.ingest,
            &s.interner,
            a.str_or("symbol", "")
        ))
    ),
    mcp_tool!(
        "whereis",
        "WHERE DOES THIS CONTENT LIVE: which file/branch defines or mentions a symbol",
        SYM_PARAM,
        &["symbol"],
        |s, a| {
            Ok(doc_tools::call_whereis(
                &s.ingest,
                &s.interner,
                &s.root,
                a.str_or("symbol", ""),
                a.bool_or("with_history", false),
            ))
        }
    ),
    mcp_tool!(
        "callers",
        "Direct (1-hop) callers of a symbol (alias for find_referencing_symbols)",
        SYM_LOOKUP_PARAM,
        &["symbol"],
        call_callers
    ),
    mcp_tool!(
        "callees",
        "Direct (1-hop) callees of a symbol",
        SYM_LOOKUP_PARAM,
        &["symbol"],
        call_callees
    ),
];

/// Return list of all 33 MCP tools plus aliases.
#[must_use]
pub fn all_tool_definitions() -> Vec<Value> {
    MCP_TOOL_HANDLERS
        .iter()
        .map(|tool| tool.to_schema_json())
        .collect()
}
