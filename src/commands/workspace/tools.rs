//! MCP tools of the workspace group: `whereis`, `mentions`, `doc_drift`, `flags`, `owners`, `stray_content`
//! and the symbol edits `replace_symbol_body`, `insert_before_symbol`, `insert_after_symbol`.
use crate::query::commands::{
    CommandDescriptor, QueryContext, render_query_value, render_xml, resolve_state_commits,
};
use crate::query::doc_tools;
use crate::query::edit::{self, EditOp};
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

const SYM_PARAM: &[(&str, &str, &str)] = &[("symbol", "string", "Symbol name")];
const EDIT_INS_PARAMS: &[(&str, &str, &str)] = &[
    ("symbol", "string", "Symbol name"),
    ("text", "string", "Text to insert"),
    ("path", "string", "Optional file path hint"),
];

pub(super) static TOOLS: &[&CommandDescriptor] = &[
    mcp_tool!(
        "doc_drift",
        "WHICH OF THIS REPO'S DOC CLAIMS ARE NOW FALSE: verifies checkable anchors in markdown against live index",
        &[("with_history", "boolean", "Include repository history")],
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
        "flags",
        "WHAT IS BUILT BUT DARK: harvests ifndef header gates, CMake options, env reads",
        &[
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
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
        &[
            ("path", "string", "Optional file path filter"),
            (
                "detail",
                "integer",
                "Show individual files when greater than zero",
            ),
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
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
        "stray_content",
        "Lines authored on divergent branches that HEAD does not have",
        &[
            ("limit", "integer", "Maximum report items"),
            ("offset", "integer", "Report item offset"),
        ],
        &[],
        |s, a| {
            let value = doc_tools::call_stray_content(s.root);
            render_query_value(value, a, crate::serialize::serialize_stray_content_xml)
        }
    ),
    mcp_tool!(
        "whereis",
        "WHERE DOES THIS CONTENT LIVE: which file/branch defines or mentions a symbol",
        &[
            ("symbol", "string", "Symbol name"),
            ("with_history", "boolean", "Include repository history"),
            ("listing", "string", "Listing filter: defs, refs, or all"),
            ("full_text", "boolean", "Show full hit text"),
        ],
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

fn call_edit_tool(
    state: &QueryContext<'_>,
    op: EditOp,
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
