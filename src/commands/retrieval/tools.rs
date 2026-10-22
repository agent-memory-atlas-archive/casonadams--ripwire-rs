//! MCP tools of the retrieval ladder: `for`, `grep`, `connect`, `path_between`, `exemplar`, `lego`,
//! `from_trace`, `memory_recall`, `fetch_body` and `batch`.
use crate::query::commands::{
    CommandDescriptor, QueryContext, execute_batch_item, render_query_value, render_xml,
};
use crate::query::{graph as graph_tools, retrieval};
use crate::serialize::JsonValueExt;
use serde_json::{Value, json};

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

pub(super) static TOOLS: &[&CommandDescriptor] = &[
    mcp_tool!(
        "batch",
        "ONE-TURN CONTEXT SWEEP: answer up to 16 heterogeneous READ sub-queries in a single call",
        &[("queries", "array", "Array of tool call objects")],
        &["queries"],
        call_batch
    ),
    mcp_tool!(
        "connect",
        "When a task touches 2..16 named symbols, returns the minimal subgraph relating them",
        &[
            ("symbols", "string[]", "List of symbol names"),
            ("radius", "integer", "Connection search radius"),
        ],
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
        "exemplar",
        "BEFORE writing a function/method/class/struct, get repo's single best-in-class instance to imitate",
        &[
            (
                "role",
                "string",
                "Kind of symbol: fn, method, class, struct, interface"
            ),
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
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
        "fetch_body",
        "Full (or partial-range) source of a symbol's definition",
        &[
            ("symbol", "string", "Symbol name"),
            ("path", "string", "Optional file path hint"),
            ("start_line", "integer", "Optional 1-based start line"),
            ("end_line", "integer", "Optional 1-based end line"),
            ("outline", "boolean", "Show definition skeleton"),
            ("compress", "boolean", "Compress implementation source"),
        ],
        &["symbol"],
        call_body
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
            ("limit", "integer", "Maximum matching lines"),
            ("offset", "integer", "Matching line offset"),
            ("before", "integer", "Context lines before each match"),
            ("after", "integer", "Context lines after each match"),
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
                any_tier: crate::query::grep::parse_any_tier(a.opt_str("in"))
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
        "lego",
        "Interface-to-impls view: method contract plus EVERY implementor",
        &[
            ("interface", "string", "Interface, trait or base class name"),
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
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
];

fn call_for_tool(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
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

fn call_body(context: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let request = crate::query::detail::BodyRequest {
        root: context.root,
        symbols: args.req_str("symbol")?,
        path_hint: args.opt_str("path"),
        start_line: args.opt_usize("start_line"),
        end_line: args.opt_usize("end_line"),
        outline: args.bool_or("outline", false),
        compress: args.bool_or("compress", false),
    };
    render_xml(|out| {
        crate::query::detail::execute_body_query(
            &request,
            context.ingest,
            context.interner,
            context.sources,
            out,
        )
    })
}

fn call_batch(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let queries = args["queries"]
        .as_array()
        .ok_or("queries must be an array")?;
    if queries.len() > crate::query::commands::BATCH_LIMIT {
        return Err(format!(
            "batch supports at most {} queries",
            crate::query::commands::BATCH_LIMIT
        ));
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
