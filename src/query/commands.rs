use super::SourceReader;
use crate::domain::{IngestResult, Interner};
use crate::git::history::CommitInfo;
use crate::graph::Graph;
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

pub fn resolve_state_commits<'a>(
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

pub static COMMANDS: &[&CommandDescriptor] = &[
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
        "explore",
        "ONE-call task orientation: ranked symbols, files, and architecture map",
        &[("top_k", "integer", "Maximum number of symbols to include"),],
        &[],
        call_explore
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
