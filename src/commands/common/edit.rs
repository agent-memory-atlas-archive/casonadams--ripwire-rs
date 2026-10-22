//! Symbol edits shared by `--replace-symbol-body`, `--insert-before-symbol` and `--insert-after-symbol`,
//! with their MCP tools.
use crate::commands::Run;
use crate::commands::edit_plan::EditPlanArgs;
use crate::query::commands::QueryContext;
use crate::query::edit::{self, EditOp, EditTargetRequest};
use crate::serialize::JsonValueExt;
use serde_json::Value;
use std::io::{self, Write};

pub const EDIT_INS_PARAMS: &[(&str, &str, &str)] = &[
    ("symbol", "string", "Symbol name"),
    ("text", "string", "Text to insert"),
    ("path", "string", "Optional file path hint"),
];

/// Apply one symbol edit from the CLI; the payload and dry-run come from the edit-plan flags.
pub fn run_edit(run: &Run<'_>, spec: &str, op: EditOp) -> io::Result<()> {
    run.report(|ctx, out| {
        let safety = ctx.cli.plugins.decode::<EditPlanArgs>().unwrap_or_default();
        let (sym, payload) = resolve_edit_spec(spec, safety.edit_payload.as_deref())?;
        let res = edit::perform_atomic_edit_with_options(
            EditTargetRequest {
                root: ctx.root,
                op,
                symbol_name: &sym,
                path_hint: None,
                payload: &payload,
                dry_run: safety.dry_run,
            },
            ctx.ingest,
            ctx.interner,
        );
        writeln!(out, "{res}")
    })
}

pub fn resolve_edit_spec(
    spec: &str,
    payload_override: Option<&str>,
) -> io::Result<(String, String)> {
    resolve_edit_spec_with_reader(spec, payload_override, io::stdin())
}

pub fn resolve_edit_spec_with_reader<R: io::Read>(
    spec: &str,
    payload_override: Option<&str>,
    mut stdin_reader: R,
) -> io::Result<(String, String)> {
    if let Some((sym, inline_payload)) = spec.split_once(':') {
        return Ok((sym.to_string(), inline_payload.to_string()));
    }
    let raw_payload = match payload_override {
        Some("-") => {
            let mut buf = String::new();
            stdin_reader.read_to_string(&mut buf)?;
            buf
        }
        Some(p) => std::fs::read_to_string(p).unwrap_or_else(|_| p.to_string()),
        None => String::new(),
    };
    Ok((spec.to_string(), raw_payload))
}

pub fn call_edit_tool(
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
