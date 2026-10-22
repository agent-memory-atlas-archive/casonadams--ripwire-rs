use crate::query::commands::{CommandDescriptor, QueryContext, render_xml};
use crate::query::map::{MapRequest, execute_stream_xml_ranked, token_budget_bytes};
use crate::serialize::JsonValueExt;
use serde_json::Value;

macro_rules! mcp_tool {
    ($name:expr, $desc:expr, $params:expr, $req:expr, $handler:expr) => {
        &CommandDescriptor {
            name: $name,
            description: $desc,
            params: std::borrow::Cow::Borrowed($params),
            required: $req,
            handler: $handler,
        }
    };
}

fn call_explore(state: &QueryContext<'_>, args: &Value) -> Result<Value, String> {
    let mut request = MapRequest::new(state.root);
    request.rank_by = args.str_or("rank_by", request.rank_by);
    if let Some(top_k) = args.opt_usize("top_k") {
        request.top_k = Some(top_k);
    }
    if let Some(budget) = args.opt_usize("budget_tokens") {
        request.pack_budget = Some(token_budget_bytes(budget));
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

pub(super) static TOOLS: &[&CommandDescriptor] = &[
    mcp_tool!(
        "analyze",
        "Architecture map for a directory: top symbols, signatures, and direct callees",
        &[
            ("top_k", "integer", "Maximum symbols"),
            (
                "rank_by",
                "string",
                "pagerank, authority, hub, degree, ccx, stable"
            ),
        ],
        &[],
        call_explore
    ),
    mcp_tool!(
        "explore",
        "ONE-call task orientation: ranked symbols, files, and architecture map",
        &[
            ("top_k", "integer", "Maximum symbols"),
            (
                "rank_by",
                "string",
                "pagerank, authority, hub, degree, ccx, stable"
            ),
        ],
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
            ("top_k", "integer", "Maximum symbols"),
        ],
        &["rank_by"],
        call_explore
    ),
];
