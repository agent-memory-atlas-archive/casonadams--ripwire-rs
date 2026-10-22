use super::SourceReader;
use super::edit::{self, EditOp};
use super::{doc_tools, graph as graph_tools};
use crate::ToXmlString;
use crate::domain::{IngestResult, Interner};
use crate::git::history::CommitInfo;
use crate::graph::Graph;
use crate::ingest::LanguageRegistry;
use crate::query::map::{MapRequest, execute_stream_xml_ranked};
use crate::query::navigation::{NavContext, NavMode, execute_nav_symbol_query_in_root};
use crate::query::retrieval;
use crate::rank::PageRankResult;
use crate::serialize::JsonValueExt;
use serde_json::{Value, json};
/// Borrowed facts shared by all query transports.
pub struct QueryContext<'a> {
    pub root: &'a str,
    pub ingest: &'a IngestResult,
    pub graph: &'a Graph,
    pub pr: &'a PageRankResult,
    pub interner: &'a Interner,
    pub commits: Option<&'a [CommitInfo]>,
    pub sources: &'a dyn SourceReader,
    pub at: Option<&'a str>,
}

impl<'a> crate::domain::CorpusView<'a> for &QueryContext<'a> {
    fn ingest(&self) -> &'a IngestResult {
        self.ingest
    }
    fn interner(&self) -> &'a Interner {
        self.interner
    }
}

pub fn render_xml(
    render: impl FnOnce(&mut Vec<u8>) -> std::io::Result<()>,
) -> Result<Value, String> {
    let mut bytes = Vec::new();
    render(&mut bytes).map_err(|error| error.to_string())?;
    String::from_utf8(bytes)
        .map(Value::String)
        .map_err(|error| error.to_string())
}

pub fn render_query_value(
    mut value: Value,
    args: &Value,
    render: impl FnOnce(&Value, &mut Vec<u8>) -> std::io::Result<()>,
) -> Result<Value, String> {
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        return Err(error.to_string());
    }
    for field in ["limit", "offset"] {
        if let Some(number) = args.get(field).and_then(Value::as_u64) {
            value[field] = number.into();
        }
    }
    render_xml(|out| render(&value, out))
}

fn safety_context<'a>(state: &QueryContext<'a>) -> crate::query::safety::SafetyContext<'a> {
    crate::query::safety::SafetyContext {
        root: state.root,
        ingest: state.ingest,
        graph: state.graph,
        interner: state.interner,
    }
}

fn call_navigation(state: &QueryContext<'_>, args: &Value, mode: NavMode) -> Result<Value, String> {
    let symbol = args.req_str("symbol")?;
    let context = NavContext {
        ingest: state.ingest,
        graph: state.graph,
        ranks: &state.pr.ranks,
        pr_iters: Some(state.pr.iterations),
        interner: state.interner,
        page: crate::serialize::PageRequest {
            limit: args.opt_usize("limit"),
            offset: args.opt_usize("offset"),
        },
        max_depth: args.opt_usize("max_depth").map(|depth| depth as u32),
    };
    render_xml(|out| execute_nav_symbol_query_in_root(state.root, symbol, mode, &context, out))
}

pub fn call_find_symbol(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    call_navigation(state, args, NavMode::FindSymbol)
}

pub fn call_callers(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    call_navigation(state, args, NavMode::Callers)
}

pub fn call_callees(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    call_navigation(state, args, NavMode::Callees)
}

pub fn call_impact(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    call_navigation(state, args, NavMode::Impact)
}

pub fn call_uses(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    call_navigation(state, args, NavMode::Uses)
}

pub fn call_explore(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let mut request = MapRequest::new(state.root);
    request.rank_by = args.str_or("rank_by", request.rank_by);
    if let Some(top_k) = args.opt_usize("top_k") {
        request.top_k = Some(top_k);
    }
    if let Some(budget) = args.opt_usize("budget_tokens") {
        request.pack_budget = Some(super::map::token_budget_bytes(budget));
    }
    render_xml(|out| {
        execute_stream_xml_ranked(
            &request,
            state.ingest,
            state.graph,
            state.pr,
            state.interner,
            out,
        )
    })
}

fn resolve_state_commits<'a>(
    state: &QueryContext<'a>,
    months: u32,
) -> std::borrow::Cow<'a, [CommitInfo]> {
    if let Some(commits) = state.commits {
        return std::borrow::Cow::Borrowed(commits);
    }
    let cutoff = crate::git::history::resolve_cutoff_timestamp(None, months);
    std::borrow::Cow::Owned(
        crate::git::history::walk_repository_history_scoped(state.root, cutoff).unwrap_or_default(),
    )
}

pub fn call_cochange(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let commits = resolve_state_commits(state, crate::git::history::DEFAULT_COCHANGE_MONTHS);
    let context = safety_context(state);
    let options = crate::query::safety::CochangeOptions {
        since: args.opt_str("since"),
        cochange_recur: args.opt_usize("recur").map(|value| value as u32),
        cochange_groups: args.bool_or("groups", false),
        limit: args.opt_usize("limit"),
        offset: args.opt_usize("offset"),
    };
    render_xml(|out| {
        crate::query::safety::execute_cochange_with_options(
            args.opt_str("path"),
            options,
            &commits,
            &context,
            out,
        )
    })
}

pub fn call_edit_check(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let symbol = args.req_str("symbol")?;
    let context = safety_context(state);
    let page = args
        .opt_usize("limit")
        .map(|limit| (args.opt_usize("offset").unwrap_or(0), limit));
    render_xml(|out| crate::query::safety::execute_edit_check_with(symbol, page, &context, out))
}

pub fn call_affected(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let files = args.req_str("files")?;
    let context = safety_context(state);
    render_xml(|out| {
        crate::query::safety::affected_from_sources(files, &context, state.sources).write_xml(out)
    })
}

pub fn call_quality_baseline(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    render_xml(|out| {
        crate::query::safety::execute_quality_baseline_with(
            state,
            args.bool_or("allow_dirty", false),
            out,
        )
    })
}

pub fn call_quality_delta(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let options = crate::gate::quality_delta::DeltaCliOptions {
        scope: args.opt_str("scope"),
        quality_ack: args.opt_str("quality_ack"),
        ack_only: args.opt_str("ack_only"),
        show_stale: args.bool_or("show_stale", false),
    };
    render_xml(|out| {
        crate::query::safety::execute_quality_delta_with_args(options, state, out).map(|_| ())
    })
}

pub fn call_slice(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let registry = LanguageRegistry::new().map_err(|error| error.to_string())?;
    let request = crate::slice::SliceRequest {
        root: state.root,
        target: args.req_str("target")?,
        at: state.at,
        flow: args
            .str_or("flow", "fwd")
            .parse::<crate::slice::SliceFlowDir>()?,
        max_depth: args.opt_usize("max_depth").unwrap_or(8) as u32,
    };
    let context = crate::slice::SliceContext {
        ingest: state.ingest,
        interner: state.interner,
        registry: &registry,
    };
    crate::slice::query_slice(request, context, state.sources).map(Value::String)
}

pub fn call_edit_tool(
    state: &QueryContext<'_>,
    op: edit::EditOp,
    args: &Value,
    is_replace: bool,
) -> Result<Value, String> {
    let symbol = args.req_str("symbol")?;
    let payload = args.req_str(if is_replace { "new_body" } else { "text" })?;
    let path_hint = args.opt_str_any(&["path", "file"]);
    Ok(edit::perform_atomic_edit_in_root(
        state.root,
        state.ingest,
        state.interner,
        op,
        symbol,
        path_hint,
        payload,
    ))
}

pub fn call_for_tool(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let value = retrieval::call_for_task_with_options(
        state.ingest,
        state.interner,
        &state.pr.ranks,
        args.req_str("task")?,
        retrieval::ForTaskOptions {
            root: state.root,
            at: state.at,
            graph: Some(state.graph),
            sources: Some(state.sources),
            budget_tokens: args.opt_usize("budget_tokens"),
            detail: args.opt_usize("detail"),
            adaptive: args.bool_or("adaptive", false),
            in_scope: args.opt_str("in_scope"),
            sections: args.opt_str("sections"),
            signatures_only: args.bool_or("signatures_only", false),
            auto_bodies: args.bool_or("auto_bodies", false),
        },
    );
    render_query_value(value, args, crate::serialize::serialize_for_xml)
}

pub fn call_batch(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let queries = args["queries"]
        .as_array()
        .ok_or("queries must be an array")?;
    if queries.len() > BATCH_LIMIT {
        return Err(format!("batch supports at most {BATCH_LIMIT} queries"));
    }
    Ok(Value::Array(
        queries
            .iter()
            .map(|query| {
                let name = query.get("name").and_then(Value::as_str);
                let Some(name) = name else {
                    return json!({"tool": null, "error": "batch item name must be a string"});
                };
                if let Some(object) = query.as_object()
                    && object.keys().any(|key| key != "name" && key != "arguments")
                {
                    return json!({"tool": name, "error": "batch item contains unknown fields"});
                }
                execute_batch_item(state, name, query.get("arguments").unwrap_or(&json!({})))
            })
            .collect(),
    ))
}
/// Transport-neutral command metadata and implementation.
pub struct CommandDescriptor {
    pub name: &'static str,
    pub description: &'static str,
    pub params: &'static [(&'static str, &'static str, &'static str)],
    pub required: &'static [&'static str],
    pub handler: fn(&QueryContext<'_>, &Value) -> Result<Value, String>,
}

macro_rules! mcp_tool {
    ($name:expr, $desc:expr, $params:expr, $req:expr, $handler:expr) => {
        &CommandDescriptor {
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

pub static COMMANDS: &[&CommandDescriptor] = &[
    mcp_tool!(
        "affected",
        "WHICH TESTS TO RUN for a change \u{2014} test files that transitively reach the changed files/symbols",
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
                "Signal to rank by: pagerank, authority, hub, degree, ccx, stable"
            ),
        ],
        &[],
        call_explore
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
        &[],
        call_cochange
    ),
    mcp_tool!(
        "connect",
        "When a task touches 2..16 named symbols, returns the minimal subgraph relating them",
        &[("symbols", "string[]", "List of symbol names")],
        &["symbols"],
        |s, a| {
            let symbols: Vec<&str> = a["symbols"]
                .as_array()
                .ok_or("symbols must be an array")?
                .iter()
                .map(|name| name.as_str().ok_or("symbol names must be strings"))
                .collect::<Result<_, _>>()?;
            let request = graph_tools::ConnectRequest {
                root: s.root,
                symbols: &symbols,
                radius: a
                    .opt_usize("radius")
                    .unwrap_or(graph_tools::DEFAULT_CONNECT_RADIUS as usize)
                    as u32,
            };
            render_xml(|out| {
                graph_tools::execute_connect(&request, s.ingest, s.graph, s.interner, out)
            })
        }
    ),
    mcp_tool!(
        "doc_drift",
        "WHICH OF THIS REPO'S DOC CLAIMS ARE NOW FALSE: verifies checkable anchors in markdown against live index",
        &[],
        &[],
        |s, a| {
            let value = doc_tools::doc_drift_with_sources_and_history(
                s.root,
                s.sources,
                s.ingest,
                s.interner,
                a.bool_or("with_history", false),
            );
            render_query_value(value, a, crate::serialize::serialize_doc_drift_xml)
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
            let val = retrieval::call_exemplar_in_root(
                s.sources,
                s.ingest,
                s.interner,
                &s.pr.ranks,
                a.str_any(&["role", "kind"], "fn"),
                Some(s.graph),
            );
            render_query_value(val, a, |val, out| {
                crate::serialize::serialize_exemplar_xml(s.root, val, out)
            })
        }
    ),
    mcp_tool!(
        "explore",
        "ONE-call task orientation: ranked symbols, files, and architecture map",
        &[("top_k", "integer", "Maximum number of symbols to include"),],
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
        call_body
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
        |s, a| {
            let value = doc_tools::flags_from_sources(s.root, s.sources, s.ingest, s.interner);
            render_query_value(value, a, crate::serialize::serialize_flags_xml)
        }
    ),
    mcp_tool!(
        "for",
        "Task-lens ranked signatures-only inventory of building blocks most relevant to a task",
        &[
            ("task", "string", "Task description in plain words"),
            ("budget_tokens", "integer", "Approximate token ceiling"),
            ("detail", "integer", "Maximum implementation bodies"),
            ("adaptive", "boolean", "Adapt detail to task confidence"),
            ("in_scope", "string", "Path filter applied before ranking"),
            ("sections", "string", "Comma-separated report sections"),
            ("signatures_only", "boolean", "Omit implementation bodies"),
            (
                "auto_bodies",
                "boolean",
                "Include high-value implementation bodies"
            ),
        ],
        &["task"],
        call_for_tool
    ),
    mcp_tool!(
        "from_trace",
        "Paste a stack trace / compiler error and get it mapped onto indexed symbols innermost-first",
        &[("trace", "string", "Stack trace text")],
        &["trace"],
        |s, a| {
            let val = graph_tools::call_from_trace(s.ingest, s.interner, a.req_str("trace")?);
            render_xml(|out| crate::serialize::serialize_from_trace_xml(&val, out))
        }
    ),
    mcp_tool!(
        "grep",
        "Literal search; each hit annotated with its enclosing symbol",
        &[
            ("pattern", "string", "Literal pattern to search"),
            ("path", "string", "Optional path filter"),
            ("in", "string", "Span tier: code (default) or any"),
        ],
        &["pattern"],
        |s, a| {
            let corpus = s.sources.corpus();
            let req = retrieval::GrepRequest {
                corpus_paths: &corpus.rel_paths,
                pruned_dirs: corpus.pruned_dirs,
                limit: a.opt_usize("limit"),
                offset: a.opt_usize("offset").unwrap_or(0),
                path_filter: a.opt_str("path"),
                any_tier: super::grep::parse_any_tier(a.opt_str("in"))
                    .map_err(|error| error.to_string())?,
                before: a.opt_usize("before").unwrap_or(0),
                after: a.opt_usize("after").unwrap_or(0),
                graph: Some(s.graph),
                ..retrieval::GrepRequest::new(s.root, a.req_str("pattern")?, s.sources)
            };
            let val = retrieval::call_grep(s.ingest, s.interner, &req);
            render_xml(|out| crate::serialize::serialize_grep_xml(&val, out))
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
            let mut val = retrieval::call_lego(s.ingest, s.interner, a.req_str("interface")?);
            val.attach_gauges(s.graph);
            render_query_value(val, a, |val, out| {
                crate::serialize::serialize_lego_xml(s.root, val, out)
            })
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
            let inputs = retrieval::collect_recall_inputs(std::path::Path::new(s.root));
            let task = a.req_str("task")?;
            let val = retrieval::call_memory_recall(
                &inputs,
                task,
                a.opt_usize("top_k"),
                a.opt_usize("budget_tokens"),
            );
            render_xml(|out| crate::serialize::serialize_recall_text(task, &val, out))
        }
    ),
    mcp_tool!(
        "mentions",
        "Docs that name a code symbol in a backtick",
        SYM_PARAM,
        &["symbol"],
        |s, a| {
            let symbol = a.req_str("symbol")?;
            if crate::navigate::find_symbols_by_name(symbol, s.ingest, s.interner).is_empty() {
                return Err(format!("ripwire-rs: --mentions symbol not found: {symbol}"));
            }
            let value = doc_tools::mentions_from_sources(s.sources, s.ingest, s.interner, symbol);
            render_xml(|out| crate::serialize::serialize_mentions_xml(&value, out))
        }
    ),
    mcp_tool!(
        "owners",
        "Bus-factor: recency-weighted author ownership per file",
        &[("path", "string", "Optional file path filter")],
        &[],
        |s, a| {
            let commits = resolve_state_commits(s, 12);
            let mut value = doc_tools::call_owners(
                Some(&commits),
                a.opt_str_any(&["path", "file"]),
                s.at,
                None,
            );
            value["root"] = s.root.into();
            value["detail"] = a
                .opt_usize("detail")
                .is_some_and(|detail| detail > 0)
                .into();
            render_query_value(value, a, crate::serialize::serialize_owners_xml)
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
            let request = graph_tools::PathRequest {
                from: a.req_str("from")?,
                to: a.req_str("to")?,
            };
            render_xml(|out| {
                graph_tools::execute_path_between(&request, s.ingest, s.graph, s.interner, out)
            })
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
        "Architecture map ranked by the same signals as CLI --rank-by",
        &[
            (
                "rank_by",
                "string",
                "pagerank, authority, hub, degree, ccx, stable"
            ),
            ("top_k", "integer", "Maximum symbols to include"),
        ],
        &["rank_by"],
        call_explore
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
            let files = a.str_vec("files");
            let report = crate::query::safety::SituationalReport {
                root: s.root,
                files: &files,
                commits: Some(&commits),
                context: crate::query::safety::SafetyContext {
                    root: s.root,
                    ingest: s.ingest,
                    graph: s.graph,
                    interner: s.interner,
                },
            };
            use crate::serialize::XmlReport;
            render_xml(|out| report.render_envelope(out))
        }
    ),
    mcp_tool!(
        "slice",
        "ARISE intra-procedural def-use variable dataflow slice inside a function",
        &[
            ("target", "string", "sym:var or bare sym to list locals"),
            ("flow", "string", "fwd, back, or both"),
            ("max_depth", "integer", "Maximum dataflow depth (default 8)"),
        ],
        &["target"],
        call_slice
    ),
    mcp_tool!(
        "stray_content",
        "Lines authored on divergent branches that HEAD does not have",
        &[],
        &[],
        |s, a| {
            let value = doc_tools::call_stray_content(s.root);
            render_query_value(value, a, crate::serialize::serialize_stray_content_xml)
        }
    ),
    mcp_tool!(
        "uses",
        "All statically resolvable use-sites of a symbol: call, read, write, import, extends",
        SYM_PARAM,
        &["symbol"],
        call_uses
    ),
    mcp_tool!(
        "whereis",
        "WHERE DOES THIS CONTENT LIVE: which file/branch defines or mentions a symbol",
        SYM_PARAM,
        &["symbol"],
        |s, a| {
            doc_tools::validate_whereis_listing(a.opt_str("listing"))?;
            let mut value = doc_tools::call_whereis(
                s.ingest,
                s.interner,
                s.root,
                a.req_str("symbol")?,
                a.bool_or("with_history", false),
            );
            for field in ["listing", "full_text"] {
                if let Some(selector) = a.get(field) {
                    value[field] = selector.clone();
                }
            }
            render_query_value(value, a, crate::serialize::serialize_whereis_xml)
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

pub fn call_body(context: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let request = super::detail::BodyRequest {
        root: context.root,
        symbols: args.req_str("symbol")?,
        path_hint: args.opt_str("path"),
        start_line: args.opt_usize("start_line"),
        end_line: args.opt_usize("end_line"),
        outline: args.bool_or("outline", false),
        compress: args.bool_or("compress", false),
    };
    render_xml(|out| {
        super::detail::execute_body_query(
            &request,
            context.ingest,
            context.interner,
            context.sources,
            out,
        )
    })
}

impl CommandDescriptor {
    pub fn parameters(&self) -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
        self.params
            .iter()
            .copied()
            .chain(extra_parameters(self.name).iter().copied())
    }
}

fn extra_parameters(name: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    match name {
        "find_symbol" | "find_referencing_symbols" | "callers" | "callees" | "uses" | "impact" => {
            &[
                ("limit", "integer", "Maximum report items"),
                ("offset", "integer", "Report item offset"),
                ("max_depth", "integer", "Maximum traversal depth"),
            ]
        }
        "analyze" | "explore" | "rank_by" => &[
            (
                "rank_by",
                "string",
                "pagerank, authority, hub, degree, ccx, stable",
            ),
            ("budget_tokens", "integer", "Approximate token ceiling"),
            ("top_k", "integer", "Maximum symbols"),
        ],
        "cochange" => &[
            ("recur", "integer", "Recursive co-change depth"),
            ("groups", "boolean", "Report co-change groups"),
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
        "connect" => &[("radius", "integer", "Connection search radius")],
        "doc_drift" => &[("with_history", "boolean", "Include repository history")],
        "whereis" => &[
            ("with_history", "boolean", "Include repository history"),
            ("listing", "string", "Listing filter: defs, refs, or all"),
            ("full_text", "boolean", "Show full hit text"),
        ],
        "owners" => &[
            (
                "detail",
                "integer",
                "Show individual files when greater than zero",
            ),
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
        "edit_check" | "lego" | "exemplar" | "flags" | "stray_content" => &[
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
        "fetch_body" => &[
            ("outline", "boolean", "Show definition skeleton"),
            ("compress", "boolean", "Compress implementation source"),
        ],
        "grep" => &[
            ("limit", "integer", "Maximum matching lines"),
            ("offset", "integer", "Matching line offset"),
            ("before", "integer", "Context lines before each match"),
            ("after", "integer", "Context lines after each match"),
        ],
        "quality_delta" => &[
            ("scope", "string", "Quality delta scope"),
            ("quality_ack", "string", "Acknowledgement text"),
            ("ack_only", "string", "Acknowledgement finding patterns"),
            ("show_stale", "boolean", "Include stale findings"),
        ],
        _ => &[],
    }
}

pub fn validate_arguments(command: &CommandDescriptor, args: &Value) -> Result<(), String> {
    let object = args.as_object().ok_or("arguments must be an object")?;
    for required in command.required {
        if !object.contains_key(*required) {
            return Err(format!("missing required argument '{required}'"));
        }
    }
    for (name, value) in object {
        let (_, kind, _) = command
            .parameters()
            .find(|(parameter, _, _)| *parameter == name)
            .ok_or_else(|| format!("unknown argument '{name}'"))?;
        validate_argument_type(name, kind, value)?;
    }
    Ok(())
}

fn validate_argument_type(name: &str, kind: &str, value: &Value) -> Result<(), String> {
    let valid = match kind {
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "integer" => value
            .as_u64()
            .is_some_and(|number| usize::try_from(number).is_ok()),
        "array" => value.is_array(),
        "string[]" => value
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_string)),
        _ => false,
    };
    if !valid {
        return Err(format!("argument '{name}' must be {kind}"));
    }
    if matches!(name, "max_depth" | "radius" | "recur")
        && value
            .as_u64()
            .is_some_and(|number| number > u32::MAX as u64)
    {
        return Err(format!("argument '{name}' exceeds the supported depth"));
    }
    Ok(())
}

/// Dispatch a validated command without transport or session state.
pub fn dispatch(context: &QueryContext<'_>, name: &str, args: &Value) -> Result<Value, String> {
    let command = COMMANDS
        .iter()
        .find(|command| command.name == name)
        .ok_or_else(|| format!("unknown query: {name}"))?;
    validate_arguments(command, args)?;
    (command.handler)(context, args)
}

pub const BATCH_LIMIT: usize = 16;

#[derive(serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BatchResultKind {
    Xml,
    Text,
    Json,
}

pub struct BatchQuery {
    pub name: String,
    pub arguments: Value,
}

fn batch_read_only(name: &str, arguments: &Value) -> bool {
    !matches!(
        name,
        "batch"
            | "quality_baseline"
            | "insert_after_symbol"
            | "insert_before_symbol"
            | "replace_symbol_body"
    ) && !(name == "quality_delta"
        && (arguments.get("quality_ack").is_some() || arguments.get("ack_only").is_some()))
}

pub fn execute_batch(
    context: &QueryContext<'_>,
    queries: &[BatchQuery],
) -> Result<Vec<Value>, String> {
    if queries.len() > BATCH_LIMIT {
        return Err(format!("batch supports at most {BATCH_LIMIT} queries"));
    }
    Ok(queries
        .iter()
        .map(|query| execute_batch_item(context, &query.name, &query.arguments))
        .collect())
}

fn execute_batch_item(context: &QueryContext<'_>, name: &str, arguments: &Value) -> Value {
    let result = if batch_read_only(name, arguments) {
        dispatch(context, name, arguments)
    } else {
        Err(format!(
            "batch permits only non-recursive read queries: {name}"
        ))
    };
    match result {
        Ok(value) => {
            let kind = if !value.is_string() {
                BatchResultKind::Json
            } else if name == "memory_recall" {
                BatchResultKind::Text
            } else {
                BatchResultKind::Xml
            };
            json!({"tool": name, "result": value, "kind": kind})
        }
        Err(error) => json!({"tool": name, "error": error}),
    }
}

/// Parse the CLI line format, also accepting a JSON argument object after `name:`.
pub fn parse_batch_content(content: &str) -> Vec<BatchQuery> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .take(BATCH_LIMIT + 1)
        .map(|line| {
            let (name, text) = line.split_once(':').unwrap_or((line, ""));
            let name = name.trim();
            let text = text.trim();
            let arguments = if text.starts_with('{') {
                serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
            } else {
                let key = match name {
                    "grep" => "pattern",
                    "for" | "memory_recall" => "task",
                    "slice" => "target",
                    "cochange" | "owners" => "path",
                    "affected" => "files",
                    "from_trace" => "trace",
                    "lego" => "interface",
                    "exemplar" => "role",
                    _ => "symbol",
                };
                if text.is_empty() {
                    json!({})
                } else {
                    json!({ (key): text })
                }
            };
            BatchQuery {
                name: name.to_string(),
                arguments,
            }
        })
        .collect()
}
