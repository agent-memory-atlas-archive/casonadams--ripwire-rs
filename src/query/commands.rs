use super::SourceReader;
use super::edit::{self, EditOp};
use super::doc_tools;
use crate::ToXmlString;
use crate::domain::{IngestResult, Interner};
use crate::git::history::CommitInfo;
use crate::graph::Graph;
use crate::ingest::LanguageRegistry;
use crate::query::map::{MapRequest, execute_stream_xml_ranked};
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
        "explore",
        "ONE-call task orientation: ranked symbols, files, and architecture map",
        &[("top_k", "integer", "Maximum number of symbols to include"),],
        &[],
        call_explore
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
];

/// Every MCP tool: commands not yet owned by a plugin plus every plugin's, sorted by name.
#[must_use]
pub fn all_commands() -> Vec<&'static CommandDescriptor> {
    let mut all: Vec<_> = COMMANDS
        .iter()
        .copied()
        .chain(crate::commands::tools())
        .collect();
    all.sort_by_key(|command| command.name);
    all
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
        "edit_check" | "flags" | "stray_content" => &[
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
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
    let command = all_commands()
        .into_iter()
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

pub fn execute_batch_item(context: &QueryContext<'_>, name: &str, arguments: &Value) -> Value {
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
