use super::dispatch::{
    McpToolHandler, call_affected, call_analyze_tool, call_batch, call_callees, call_callers,
    call_cochange, call_connect_tool, call_doc_drift_tool, call_edit_check, call_edit_tool,
    call_exemplar_tool, call_explore, call_fetch_body_tool, call_find_symbol, call_flags_tool,
    call_for_tool, call_from_trace_tool, call_grep_tool, call_impact, call_lego_tool,
    call_memory_recall_tool, call_mentions_tool, call_owners_tool, call_path_between_tool,
    call_quality_baseline, call_quality_delta, call_rank_by_tool, call_situational_awareness_tool,
    call_slice, call_stray_content_tool, call_uses_tool, call_whereis_tool,
};
use super::edit::EditOp;
use super::state::McpState;
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

pub struct AffectedTool;
impl McpToolHandler for AffectedTool {
    fn name(&self) -> &'static str {
        "affected"
    }
    fn description(&self) -> &'static str {
        "WHICH TESTS TO RUN for a change — test files that transitively reach the changed files/symbols"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[(
            "files",
            "string",
            "Changed files or symbols, comma-separated",
        )]
    }
    fn required(&self) -> &'static [&'static str] {
        &["files"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_affected(state, args)
    }
}

pub struct AnalyzeTool;
impl McpToolHandler for AnalyzeTool {
    fn name(&self) -> &'static str {
        "analyze"
    }
    fn description(&self) -> &'static str {
        "Architecture map for a directory: top symbols, signatures, and direct callees"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("top_k", "integer", "Maximum number of symbols"),
            (
                "rank_by",
                "string",
                "Signal to rank by: pagerank, degree, ccx, churn",
            ),
        ]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_analyze_tool(state, args)
    }
}

pub struct BatchTool;
impl McpToolHandler for BatchTool {
    fn name(&self) -> &'static str {
        "batch"
    }
    fn description(&self) -> &'static str {
        "ONE-TURN CONTEXT SWEEP: answer up to 16 heterogeneous READ sub-queries in a single call"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("queries", "array", "Array of tool call objects")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["queries"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_batch(state, args)
    }
}

pub struct CochangeTool;
impl McpToolHandler for CochangeTool {
    fn name(&self) -> &'static str {
        "cochange"
    }
    fn description(&self) -> &'static str {
        "Files that historically change together with this file (Fowler's Shotgun Surgery logical coupling)"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            (
                "path",
                "string",
                "Target file path to find co-change partners for",
            ),
            (
                "since",
                "string",
                "Time window or revision (e.g. '18 months ago')",
            ),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["path"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_cochange(state, args)
    }
}

pub struct ConnectTool;
impl McpToolHandler for ConnectTool {
    fn name(&self) -> &'static str {
        "connect"
    }
    fn description(&self) -> &'static str {
        "When a task touches 2..16 named symbols, returns the minimal subgraph relating them"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("symbols", "string[]", "List of symbol names")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbols"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_connect_tool(state, args)
    }
}

pub struct DocDriftTool;
impl McpToolHandler for DocDriftTool {
    fn name(&self) -> &'static str {
        "doc_drift"
    }
    fn description(&self) -> &'static str {
        "WHICH OF THIS REPO'S DOC CLAIMS ARE NOW FALSE: verifies checkable anchors in markdown against live index"
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_doc_drift_tool(state, args)
    }
}

pub struct EditCheckTool;
impl McpToolHandler for EditCheckTool {
    fn name(&self) -> &'static str {
        "edit_check"
    }
    fn description(&self) -> &'static str {
        "After an edit: did SYM's contract change, and which callers no longer fit?"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("symbol", "string", "Symbol name to verify")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_edit_check(state, args)
    }
}

pub struct ExemplarTool;
impl McpToolHandler for ExemplarTool {
    fn name(&self) -> &'static str {
        "exemplar"
    }
    fn description(&self) -> &'static str {
        "BEFORE writing a function/method/class/struct, get repo's single best-in-class instance to imitate"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[(
            "role",
            "string",
            "Kind of symbol: fn, method, class, struct, interface",
        )]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_exemplar_tool(state, args)
    }
}

pub struct ExploreTool;
impl McpToolHandler for ExploreTool {
    fn name(&self) -> &'static str {
        "explore"
    }
    fn description(&self) -> &'static str {
        "ONE-call task orientation: ranked symbols, files, and architecture map"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("path", "string", "Repository path to explore"),
            ("top_k", "integer", "Maximum number of symbols to include"),
        ]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_explore(state, args)
    }
}

pub struct FetchBodyTool;
impl McpToolHandler for FetchBodyTool {
    fn name(&self) -> &'static str {
        "fetch_body"
    }
    fn description(&self) -> &'static str {
        "Full (or partial-range) source of a symbol's definition"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("symbol", "string", "Symbol name"),
            ("path", "string", "Optional file path hint"),
            ("start_line", "integer", "Optional 1-based start line"),
            ("end_line", "integer", "Optional 1-based end line"),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_fetch_body_tool(state, args)
    }
}

pub struct FindReferencingSymbolsTool;
impl McpToolHandler for FindReferencingSymbolsTool {
    fn name(&self) -> &'static str {
        "find_referencing_symbols"
    }
    fn description(&self) -> &'static str {
        "Direct (1-hop) callers of a symbol"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("symbol", "string", "Symbol name to look up")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_callers(state, args)
    }
}

pub struct FindSymbolTool;
impl McpToolHandler for FindSymbolTool {
    fn name(&self) -> &'static str {
        "find_symbol"
    }
    fn description(&self) -> &'static str {
        "A symbol's 1-hop neighborhood: the symbol plus direct callers and callees"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("symbol", "string", "Symbol name to look up")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_find_symbol(state, args)
    }
}

pub struct FlagsTool;
impl McpToolHandler for FlagsTool {
    fn name(&self) -> &'static str {
        "flags"
    }
    fn description(&self) -> &'static str {
        "WHAT IS BUILT BUT DARK: harvests ifndef header gates, CMake options, env reads"
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_flags_tool(state, args)
    }
}

pub struct ForTool;
impl McpToolHandler for ForTool {
    fn name(&self) -> &'static str {
        "for"
    }
    fn description(&self) -> &'static str {
        "Task-lens ranked signatures-only inventory of building blocks most relevant to a task"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("task", "string", "Task description in plain words"),
            ("budget_tokens", "integer", "Maximum number of symbols"),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["task"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_for_tool(state, args)
    }
}

pub struct FromTraceTool;
impl McpToolHandler for FromTraceTool {
    fn name(&self) -> &'static str {
        "from_trace"
    }
    fn description(&self) -> &'static str {
        "Paste a stack trace / compiler error and get it mapped onto indexed symbols innermost-first"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("trace", "string", "Stack trace text")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["trace"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_from_trace_tool(state, args)
    }
}

pub struct GrepTool;
impl McpToolHandler for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }
    fn description(&self) -> &'static str {
        "Literal search; each hit annotated with its enclosing symbol"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("pattern", "string", "Literal pattern to search"),
            ("path", "string", "Optional path filter"),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["pattern"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_grep_tool(state, args)
    }
}

pub struct ImpactTool;
impl McpToolHandler for ImpactTool {
    fn name(&self) -> &'static str {
        "impact"
    }
    fn description(&self) -> &'static str {
        "Transitive blast radius of a symbol via calls, ranked by PageRank"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            (
                "symbol",
                "string",
                "Symbol name to analyze blast radius for",
            ),
            ("max_depth", "integer", "Maximum hop depth"),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_impact(state, args)
    }
}

pub struct InsertAfterSymbolTool;
impl McpToolHandler for InsertAfterSymbolTool {
    fn name(&self) -> &'static str {
        "insert_after_symbol"
    }
    fn description(&self) -> &'static str {
        "Insert text immediately AFTER a symbol's definition (atomic write-rename)"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("symbol", "string", "Symbol name"),
            ("text", "string", "Text to insert"),
            ("path", "string", "Optional file path hint"),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol", "text"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_edit_tool(state, EditOp::InsertAfter, args, false)
    }
}

pub struct InsertBeforeSymbolTool;
impl McpToolHandler for InsertBeforeSymbolTool {
    fn name(&self) -> &'static str {
        "insert_before_symbol"
    }
    fn description(&self) -> &'static str {
        "Insert text immediately BEFORE a symbol's definition (atomic write-rename)"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("symbol", "string", "Symbol name"),
            ("text", "string", "Text to insert"),
            ("path", "string", "Optional file path hint"),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol", "text"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_edit_tool(state, EditOp::InsertBefore, args, false)
    }
}

pub struct LegoTool;
impl McpToolHandler for LegoTool {
    fn name(&self) -> &'static str {
        "lego"
    }
    fn description(&self) -> &'static str {
        "Interface-to-impls view: method contract plus EVERY implementor"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("interface", "string", "Interface, trait or base class name")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["interface"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_lego_tool(state, args)
    }
}

pub struct MemoryRecallTool;
impl McpToolHandler for MemoryRecallTool {
    fn name(&self) -> &'static str {
        "memory_recall"
    }
    fn description(&self) -> &'static str {
        "Most relevant memory notes / docs for a task, full text"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("task", "string", "Task description"),
            ("top_k", "integer", "Maximum number of notes"),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["task"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_memory_recall_tool(state, args)
    }
}

pub struct MentionsTool;
impl McpToolHandler for MentionsTool {
    fn name(&self) -> &'static str {
        "mentions"
    }
    fn description(&self) -> &'static str {
        "Docs that name a code symbol in a backtick"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("symbol", "string", "Symbol name")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_mentions_tool(state, args)
    }
}

pub struct OwnersTool;
impl McpToolHandler for OwnersTool {
    fn name(&self) -> &'static str {
        "owners"
    }
    fn description(&self) -> &'static str {
        "Bus-factor: recency-weighted author ownership per file"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("path", "string", "Optional file path filter")]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_owners_tool(state, args)
    }
}

pub struct PathBetweenTool;
impl McpToolHandler for PathBetweenTool {
    fn name(&self) -> &'static str {
        "path_between"
    }
    fn description(&self) -> &'static str {
        "Does A REACH B, and HOW? Shortest directed call path between two symbols"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("from", "string", "Source symbol name"),
            ("to", "string", "Target symbol name"),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["from", "to"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_path_between_tool(state, args)
    }
}

pub struct QualityBaselineTool;
impl McpToolHandler for QualityBaselineTool {
    fn name(&self) -> &'static str {
        "quality_baseline"
    }
    fn description(&self) -> &'static str {
        "PIN the quality floor: writes .ripwire-rs_quality_baseline stamped with git HEAD sha"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[(
            "allow_dirty",
            "boolean",
            "Pin even when the dirty tree already holds gating findings against HEAD; the count is stamped as absorbed",
        )]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_quality_baseline(state, args)
    }
}

pub struct QualityDeltaTool;
impl McpToolHandler for QualityDeltaTool {
    fn name(&self) -> &'static str {
        "quality_delta"
    }
    fn description(&self) -> &'static str {
        "Your PR self-check: reports ONLY what your working tree made WORSE vs baseline"
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_quality_delta(state, args)
    }
}

pub struct RankByTool;
impl McpToolHandler for RankByTool {
    fn name(&self) -> &'static str {
        "rank_by"
    }
    fn description(&self) -> &'static str {
        "Architecture map ranked by a signal: pagerank, degree, ccx, churn"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("rank_by", "string", "pagerank, degree, ccx, churn"),
            ("top_k", "integer", "Maximum symbols to include"),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["rank_by"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_rank_by_tool(state, args)
    }
}

pub struct ReplaceSymbolBodyTool;
impl McpToolHandler for ReplaceSymbolBodyTool {
    fn name(&self) -> &'static str {
        "replace_symbol_body"
    }
    fn description(&self) -> &'static str {
        "Replace a symbol's ENTIRE definition with new_body (atomic write-rename)"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("symbol", "string", "Symbol name"),
            ("new_body", "string", "Replacement definition"),
            ("path", "string", "Optional file path hint"),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol", "new_body"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_edit_tool(state, EditOp::ReplaceBody, args, true)
    }
}

pub struct SituationalAwarenessTool;
impl McpToolHandler for SituationalAwarenessTool {
    fn name(&self) -> &'static str {
        "situational_awareness"
    }
    fn description(&self) -> &'static str {
        "The 5 things to know about a diff: blast_radius, tests_to_run, forgotten co-change files, hotspots"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("files", "string[]", "Changed file paths")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["files"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_situational_awareness_tool(state, args)
    }
}

pub struct SliceTool;
impl McpToolHandler for SliceTool {
    fn name(&self) -> &'static str {
        "slice"
    }
    fn description(&self) -> &'static str {
        "ARISE intra-procedural def-use variable dataflow slice inside a function"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("target", "string", "sym:var or bare sym to list locals"),
            ("flow", "string", "fwd, back, or both"),
        ]
    }
    fn required(&self) -> &'static [&'static str] {
        &["target"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_slice(state, args)
    }
}

pub struct StrayContentTool;
impl McpToolHandler for StrayContentTool {
    fn name(&self) -> &'static str {
        "stray_content"
    }
    fn description(&self) -> &'static str {
        "Lines authored on divergent branches that HEAD does not have"
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_stray_content_tool(state, args)
    }
}

pub struct UsesTool;
impl McpToolHandler for UsesTool {
    fn name(&self) -> &'static str {
        "uses"
    }
    fn description(&self) -> &'static str {
        "All statically resolvable use-sites of a symbol: call, read, write, import, extends"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("symbol", "string", "Symbol name")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_uses_tool(state, args)
    }
}

pub struct WhereisTool;
impl McpToolHandler for WhereisTool {
    fn name(&self) -> &'static str {
        "whereis"
    }
    fn description(&self) -> &'static str {
        "WHERE DOES THIS CONTENT LIVE: which file/branch defines or mentions a symbol"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("symbol", "string", "Symbol name")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_whereis_tool(state, args)
    }
}

pub struct CallersTool;
impl McpToolHandler for CallersTool {
    fn name(&self) -> &'static str {
        "callers"
    }
    fn description(&self) -> &'static str {
        "Direct (1-hop) callers of a symbol (alias for find_referencing_symbols)"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("symbol", "string", "Symbol name to look up")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_callers(state, args)
    }
}

pub struct CalleesTool;
impl McpToolHandler for CalleesTool {
    fn name(&self) -> &'static str {
        "callees"
    }
    fn description(&self) -> &'static str {
        "Direct (1-hop) callees of a symbol"
    }
    fn params(&self) -> &'static [(&'static str, &'static str, &'static str)] {
        &[("symbol", "string", "Symbol name to look up")]
    }
    fn required(&self) -> &'static [&'static str] {
        &["symbol"]
    }
    fn execute(&self, state: &McpState, args: &Value) -> Result<Value, String> {
        call_callees(state, args)
    }
}

pub static MCP_TOOL_HANDLERS: &[&dyn McpToolHandler] = &[
    &AffectedTool,
    &AnalyzeTool,
    &BatchTool,
    &CochangeTool,
    &ConnectTool,
    &DocDriftTool,
    &EditCheckTool,
    &ExemplarTool,
    &ExploreTool,
    &FetchBodyTool,
    &FindReferencingSymbolsTool,
    &FindSymbolTool,
    &FlagsTool,
    &ForTool,
    &FromTraceTool,
    &GrepTool,
    &ImpactTool,
    &InsertAfterSymbolTool,
    &InsertBeforeSymbolTool,
    &LegoTool,
    &MemoryRecallTool,
    &MentionsTool,
    &OwnersTool,
    &PathBetweenTool,
    &QualityBaselineTool,
    &QualityDeltaTool,
    &RankByTool,
    &ReplaceSymbolBodyTool,
    &SituationalAwarenessTool,
    &SliceTool,
    &StrayContentTool,
    &UsesTool,
    &WhereisTool,
    &CallersTool,
    &CalleesTool,
];

/// Return list of all 33 MCP tools plus aliases.
#[must_use]
pub fn all_tool_definitions() -> Vec<Value> {
    MCP_TOOL_HANDLERS
        .iter()
        .map(|tool| tool.to_schema_json())
        .collect()
}
