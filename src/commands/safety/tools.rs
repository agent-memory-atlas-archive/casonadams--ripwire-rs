//! MCP tools of the safety group: `affected`, `cochange`, `edit_check`, `quality_baseline`,
//! `quality_delta`, `situational_awareness` and `slice`.
use crate::ingest::LanguageRegistry;
use crate::ToXmlString;
use crate::query::commands::{CommandDescriptor, QueryContext, render_xml, resolve_state_commits};
use crate::serialize::JsonValueExt;
use serde_json::Value;

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
            ("recur", "integer", "Recursive co-change depth"),
            ("groups", "boolean", "Report co-change groups"),
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
        &[],
        call_cochange
    ),
    mcp_tool!(
        "edit_check",
        "After an edit: did SYM's contract change, and which callers no longer fit?",
        &[
            ("symbol", "string", "Symbol name to verify"),
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
        &["symbol"],
        call_edit_check
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
        &[
            ("scope", "string", "Quality delta scope"),
            ("quality_ack", "string", "Acknowledgement text"),
            ("ack_only", "string", "Acknowledgement finding patterns"),
            ("show_stale", "boolean", "Include stale findings"),
        ],
        &[],
        call_quality_delta
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
];

fn safety_context<'a>(state: &QueryContext<'a>) -> crate::query::safety::SafetyContext<'a> {
    crate::query::safety::SafetyContext {
        root: state.root,
        ingest: state.ingest,
        graph: state.graph,
        interner: state.interner,
    }
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
