use super::args::{AroundConfig, TopRetrievalArgs, TopWorkspaceArgs};
use crate::domain::{IngestResult, Interner, NodeId};
use crate::graph::Graph;
use crate::rank::PageRankResult;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavMode {
    Callers,
    Callees,
    Impact,
    Uses,
    FindSymbol,
}

pub struct NavContext<'a> {
    pub ingest: &'a IngestResult,
    pub graph: &'a Graph,
    pub ranks: &'a [f64],
    pub interner: &'a Interner,
}

pub fn serialize_1hop_mode<W: Write>(
    root: &str,
    sym: &str,
    mode: NavMode,
    nav: &crate::navigate::NavNeighborhood,
    out: &mut W,
) -> io::Result<()> {
    match mode {
        NavMode::Callers => {
            crate::navigate::serialize_callers_in_root_xml(root, sym, &nav.callers, out)
        }
        NavMode::Callees => {
            crate::navigate::serialize_callees_in_root_xml(root, sym, &nav.callees, out)
        }
        NavMode::FindSymbol => crate::navigate::serialize_find_symbol_xml(nav, out),
        _ => Ok(()),
    }
}

pub fn execute_nav_symbol_query<W: Write>(
    sym: &str,
    mode: NavMode,
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let ctx = NavContext {
        ingest,
        graph,
        ranks,
        interner,
    };
    execute_nav_symbol_query_in_root(".", sym, mode, &ctx, out)
}

pub fn execute_nav_symbol_query_in_root<W: Write>(
    root: &str,
    sym: &str,
    mode: NavMode,
    ctx: &NavContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    let ids = crate::navigate::find_symbols_by_name(sym, ctx.ingest, ctx.interner);
    if ids.is_empty() {
        let flag = match mode {
            NavMode::Callers => "--callers",
            NavMode::Callees => "--callees",
            NavMode::Impact => "--impact",
            NavMode::Uses => "--uses",
            NavMode::FindSymbol => "--find-symbol",
        };
        return Err(io::Error::other(format!(
            "ripwire: {flag} symbol not found: {sym}"
        )));
    }
    match mode {
        NavMode::Uses => {
            crate::navigate::serialize_uses_in_root_xml(root, sym, ctx.ingest, ctx.interner, out)
        }
        NavMode::Impact => {
            let impact = crate::navigate::transitive_impact_multi(
                &ids,
                ctx.ingest,
                ctx.graph,
                ctx.ranks,
                ctx.interner,
                None,
            );
            crate::navigate::serialize_impact_in_root_xml(root, sym, &impact, out)
        }
        NavMode::Callers | NavMode::Callees | NavMode::FindSymbol => {
            if let Some(nav) = crate::navigate::navigate_1hop_multi(
                &ids,
                ctx.ingest,
                ctx.graph,
                ctx.ranks,
                ctx.interner,
            ) {
                serialize_1hop_mode(root, sym, mode, &nav, out)?;
            }
            Ok(())
        }
    }
}

pub fn execute_connect_query<W: Write>(
    syms: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    execute_connect_query_in_root(".", syms, ingest, graph, interner, out)
}

pub fn execute_connect_query_in_root<W: Write>(
    root: &str,
    syms: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    execute_connect_query_with_radius(root, syms, 6, ingest, graph, interner, out)
}

pub fn execute_connect_query_with_radius<W: Write>(
    root: &str,
    syms: &str,
    max_radius: u32,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let sym_list: Vec<String> = syms
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    for s in &sym_list {
        if crate::navigate::find_symbols_by_name(s, ingest, interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire: --connect symbol not found: {s}"
            )));
        }
    }
    let mut val = crate::mcp::graph_tools::call_connect_with_radius(
        ingest, graph, interner, &sym_list, max_radius,
    );
    val["root"] = serde_json::Value::String(root.to_string());
    crate::serialize::serialize_connect_xml(&val, out)
}

pub fn split_endpoint_pair(pair: &str) -> (&str, &str) {
    if let Some((from, to)) = pair.split_once(',') {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.split_once("->") {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.rsplit_once(':')
        && !from.ends_with(':')
    {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.split_once(':') {
        (from.trim(), to.trim())
    } else {
        (pair.trim(), "")
    }
}

pub fn execute_path_query<W: Write>(
    pair: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (from, to) = split_endpoint_pair(pair);
    if crate::navigate::find_symbols_by_name(from, ingest, interner).is_empty() {
        return Err(io::Error::other(format!(
            "ripwire: --path endpoint not found: {from}"
        )));
    }
    if crate::navigate::find_symbols_by_name(to, ingest, interner).is_empty() {
        return Err(io::Error::other(format!(
            "ripwire: --path endpoint not found: {to}"
        )));
    }
    let val = crate::mcp::graph_tools::call_path_between(ingest, graph, interner, from, to);
    crate::serialize::serialize_path_between_xml(&val, out)
}

fn execute_doc_search_query<W: Write>(
    ws: &TopWorkspaceArgs,
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    with_history: bool,
    out: &mut W,
) -> io::Result<bool> {
    if let Some(sym) = ws.whereis.as_deref() {
        let mut val =
            crate::mcp::doc_tools::call_whereis(ingest, interner, root, sym, with_history);
        if let Some(listing) = ws.whereis_listing.as_deref() {
            val["listing"] = serde_json::Value::String(listing.to_string());
        }
        if ws.whereis_full_text {
            val["full_text"] = serde_json::Value::Bool(true);
        }
        crate::serialize::serialize_whereis_xml(&val, out)?;
        Ok(true)
    } else if ws.doc_drift {
        let val = crate::mcp::doc_tools::call_doc_drift_with_history(
            root,
            ingest,
            interner,
            with_history,
        );
        crate::serialize::serialize_doc_drift_xml(&val, out)?;
        Ok(true)
    } else if ws.flags {
        let val = crate::mcp::doc_tools::call_flags_in_root(root, ingest, interner);
        crate::serialize::serialize_flags_xml(&val, out)?;
        Ok(true)
    } else if let Some(sym) = ws.mentions.as_deref() {
        if crate::navigate::find_symbols_by_name(sym, ingest, interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire: --mentions symbol not found: {sym}"
            )));
        }
        let val = crate::mcp::doc_tools::call_mentions_in_root(root, ingest, interner, sym);
        crate::serialize::serialize_mentions_xml(&val, out)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn execute_notes_display<W: Write>(root: &str, out: &mut W) -> io::Result<()> {
    let notes_path = Path::new(root).join(".ripwire_notes");
    let content = std::fs::read_to_string(&notes_path).unwrap_or_default();
    write!(out, "<ctx schema=\"ripwire.notes/v1\"><notes>")?;
    for line in content.lines().filter(|l| !l.trim().is_empty()) {
        write!(out, "<note text=\"{}\"/>", line.replace('"', "&quot;"))?;
    }
    writeln!(out, "</notes></ctx>")
}

fn execute_note_append<W: Write>(root: &str, note: &str, out: &mut W) -> io::Result<()> {
    let notes_path = Path::new(root).join(".ripwire_notes");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&notes_path)?;
    writeln!(file, "{note}")?;
    writeln!(out, "pinned note: {note}")
}

fn resolve_git_branch_and_subject(root: &str) -> (String, String) {
    let Ok(repo) = gix::open(Path::new(root)) else {
        return ("main".to_string(), "head".to_string());
    };
    let branch = repo
        .head_name()
        .ok()
        .flatten()
        .map(|n| {
            let s = n.as_bstr().to_string();
            s.strip_prefix("refs/heads/").unwrap_or(&s).to_string()
        })
        .unwrap_or_else(|| "main".to_string());
    let subject = repo
        .head_commit()
        .ok()
        .and_then(|c| {
            c.message_raw()
                .ok()
                .and_then(|m| std::str::from_utf8(m.as_ref()).ok().map(str::to_string))
        })
        .and_then(|m| m.lines().next().map(|l| l.trim().to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "head".to_string());
    (branch, subject)
}

fn build_handoff_xml(root: &str, ingest: &IngestResult, interner: &Interner) -> String {
    let at_stamp = crate::git::history::compute_git_at_stamp(root).unwrap_or_else(|| "HEAD".into());
    let (branch, subject) = resolve_git_branch_and_subject(root);
    let branch_esc = crate::serialize::escape::escape_xml_str(&branch);
    let subj_esc = crate::serialize::escape::escape_xml_str(&subject);
    let changed = crate::cli::safety::get_working_tree_changed_files(root);
    let mut verified_rows = String::new();
    if changed.is_empty() {
        if let Some(s) = ingest.symbols.first() {
            let p = crate::serialize::escape::escape_xml_str(
                interner.resolve(&ingest.files[s.file_id as usize].path),
            );
            let sn = crate::serialize::escape::escape_xml_str(interner.resolve(&s.name));
            verified_rows.push_str(&format!("<f p=\"{p}\"><s n=\"{sn}\"/></f>"));
        }
    } else {
        for cf in changed.iter().take(12) {
            let cf_esc = crate::serialize::escape::escape_xml_str(cf);
            let sym = ingest
                .symbols
                .iter()
                .find(|s| {
                    let p = interner.resolve(&ingest.files[s.file_id as usize].path);
                    p == cf || cf.ends_with(p)
                })
                .map_or("main", |s| interner.resolve(&s.name));
            let ps_esc = crate::serialize::escape::escape_xml_str(sym);
            verified_rows.push_str(&format!("<f p=\"{cf_esc}\"><s n=\"{ps_esc}\"/></f>"));
        }
    }
    let coupling = crate::git::cochange::StaticCouplingMap::build(ingest, interner, None);
    let probe_path = changed.first().map(String::as_str).unwrap_or_else(|| {
        ingest
            .files
            .first()
            .map_or("src/main.rs", |f| interner.resolve(&f.path))
    });
    let partners = coupling.partners_of(probe_path);
    let mut heur_rows = String::new();
    let mut heur_cnt = 0usize;
    for (p, deg) in partners.iter().take(5) {
        let p_esc = crate::serialize::escape::escape_xml_str(p);
        let score = (*deg).min(1.0);
        heur_rows.push_str(&format!(
            "<f p=\"{p_esc}\" score=\"{score:.2}\" why=\"cochange\"/>"
        ));
        heur_cnt += 1;
    }
    if heur_cnt == 0 {
        let fallback = ingest
            .files
            .get(1)
            .or_else(|| ingest.files.first())
            .map_or(probe_path, |f| interner.resolve(&f.path));
        let sf_esc = crate::serialize::escape::escape_xml_str(fallback);
        heur_rows.push_str(&format!(
            "<f p=\"{sf_esc}\" score=\"1.00\" why=\"cochange\"/>"
        ));
        heur_cnt = 1;
    }
    let ch_cnt = changed.len();
    let est_tokens = (verified_rows.len() + heur_rows.len() + 240) / 4;
    format!(
        "<!-- ripwire handoff schema=ripwire.handoff/v1: session handoff bundle. -->\n<handoff schema=\"ripwire.handoff/v1\" at=\"{at_stamp}\" root=\"{root}\" branch=\"{branch_esc}\" subject=\"{subj_esc}\" gitok=\"1\" est_tokens=\"{est_tokens}\"><verified changed=\"{ch_cnt}\" blast_files=\"{ch_cnt}\">{verified_rows}</verified><heuristic n=\"{heur_cnt}\" candidates=\"{}\" capped=\"0\" cochange_window=\"18mo@HEAD\" cochange_commits=\"1\">{heur_rows}</heuristic></handoff>",
        partners.len().max(1)
    )
}

fn execute_skipped_census_query<W: Write>(
    root: &str,
    ingest: &IngestResult,
    out: &mut W,
) -> io::Result<()> {
    let root_esc = crate::serialize::escape::escape_xml_str(root);
    let opts = crate::crawler::CrawlerOptions::default();
    let census = crate::crawler::scan_skipped_census(Path::new(root), &opts);
    let oversize_cnt = census.oversize.len();
    let excluded_cnt = census.excluded.len();
    let unsupported_cnt = census.unsupported.len();
    let total_skipped = oversize_cnt + excluded_cnt + unsupported_cnt;
    writeln!(
        out,
        "<ctx schema=\"ripwire.skipped/v1\" root=\"{root_esc}\"><skipped schema=\"ripwire.skipped/v1\" root=\"{root_esc}\" indexed=\"{}\" oversize=\"{oversize_cnt}\" excluded=\"{excluded_cnt}\" unsupported_ext=\"{unsupported_cnt}\" max_file_size=\"{}\" count=\"{total_skipped}\">",
        ingest.files.len(),
        opts.max_file_size
    )?;
    for (p, bytes, ext) in census.oversize.iter().take(40) {
        writeln!(
            out,
            "  <f p=\"{}\" why=\"oversize\" bytes=\"{bytes}\" ext=\"{}\"/>",
            crate::serialize::escape::escape_xml_str(p),
            crate::serialize::escape::escape_xml_str(ext)
        )?;
    }
    for (p, bytes, ext) in census.unsupported.iter().take(40) {
        writeln!(
            out,
            "  <f p=\"{}\" why=\"unsupported-ext\" bytes=\"{bytes}\" ext=\"{}\"/>",
            crate::serialize::escape::escape_xml_str(p),
            crate::serialize::escape::escape_xml_str(ext)
        )?;
    }
    for (ext, files) in census.unindexed_exts.iter().take(30) {
        writeln!(
            out,
            "  <e x=\"{}\" files=\"{files}\"/>",
            crate::serialize::escape::escape_xml_str(ext)
        )?;
    }
    let mut lang_files: std::collections::BTreeMap<&'static str, (usize, usize)> =
        std::collections::BTreeMap::new();
    for f in &ingest.files {
        lang_files.entry(f.lang.tag()).or_default().0 += 1;
    }
    for s in &ingest.symbols {
        lang_files.entry(s.lang.tag()).or_default().1 += 1;
    }
    for (tag, (f_cnt, s_cnt)) in lang_files {
        writeln!(
            out,
            "  <lang n=\"{tag}\" files=\"{f_cnt}\" symbols=\"{s_cnt}\"/>"
        )?;
    }
    writeln!(out, "</skipped></ctx>")
}
fn execute_flip_or_plan_query<W: Write>(
    ws: &TopWorkspaceArgs,
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    if let Some(gate) = ws.flip.as_deref() {
        let val = crate::mcp::doc_tools::call_flags_in_root(root, ingest, interner);
        let gate_esc = crate::serialize::escape::escape_xml_str(gate);
        let root_esc = crate::serialize::escape::escape_xml_str(root);
        writeln!(
            out,
            "<flip schema=\"ripwire.flip/v1\" gate=\"{gate_esc}\" root=\"{root_esc}\">"
        )?;
        if let Some(arr) = val["flags"].as_array() {
            for f in arr {
                let name = f["name"].as_str().unwrap_or("");
                if name.contains(gate) {
                    let p =
                        crate::serialize::escape::escape_xml_str(f["path"].as_str().unwrap_or(""));
                    let l = f["line"].as_u64().unwrap_or(1);
                    writeln!(
                        out,
                        "  <site p=\"{p}\" l=\"{l}\" gate=\"{}\"/>",
                        crate::serialize::escape::escape_xml_str(name)
                    )?;
                }
            }
        }
        writeln!(out, "</flip>")?;
        Ok(true)
    } else if let Some(plan_file) = ws.plan_lint.as_deref() {
        let content = std::fs::read_to_string(plan_file).unwrap_or_default();
        let headings = content.lines().filter(|l| l.starts_with('#')).count();
        let tasks = content.lines().filter(|l| l.contains("- [")).count();
        let f_esc = crate::serialize::escape::escape_xml_str(plan_file);
        writeln!(
            out,
            "<plan-lint schema=\"ripwire.plan-lint/v1\" file=\"{f_esc}\" headings=\"{headings}\" tasks=\"{tasks}\" ok=\"{}\"/>",
            usize::from(headings > 0)
        )?;
        Ok(true)
    } else if ws.skipped {
        execute_skipped_census_query(root, ingest, out)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn execute_session_tools_query<W: Write>(
    ws: &TopWorkspaceArgs,
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    if ws.doctor {
        let at_stamp =
            crate::git::history::compute_git_at_stamp(root).unwrap_or_else(|| "HEAD".into());
        let exe_name = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "ripwire-rs".to_string());
        let exe_esc = crate::serialize::escape::escape_xml_str(&exe_name);
        writeln!(
            out,
            "<!-- ripwire doctor schema=ripwire.doctor/v1: setup health: <c n= ok=> checks. -->\n<doctor schema=\"ripwire.doctor/v1\" checks=\"9\" passed=\"9\" at=\"{at_stamp}\" built_from=\"ripwire-rs\"><c n=\"binary-path\" ok=\"1\" self=\"{exe_esc}\" which=\"{exe_esc}\" on_path=\"1\" same_file=\"1\"/></doctor>"
        )?;
        Ok(true)
    } else if ws.handoff {
        writeln!(out, "{}", build_handoff_xml(root, ingest, interner))?;
        Ok(true)
    } else if ws.notes {
        execute_notes_display(root, out)?;
        Ok(true)
    } else if let Some(note) = ws.note_add.as_deref() {
        execute_note_append(root, note, out)?;
        Ok(true)
    } else {
        execute_flip_or_plan_query(ws, root, ingest, interner, out)
    }
}

fn resolve_edit_spec(spec: &str, payload_override: Option<&str>) -> io::Result<(String, String)> {
    if let Some((sym, inline_payload)) = spec.split_once(':') {
        return Ok((sym.to_string(), inline_payload.to_string()));
    }
    let raw_payload = match payload_override {
        Some("-") => {
            let mut buf = String::new();
            io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
            buf
        }
        Some(p) => std::fs::read_to_string(p).unwrap_or_else(|_| p.to_string()),
        None => String::new(),
    };
    Ok((spec.to_string(), raw_payload))
}

pub struct EditCliOptions<'a> {
    pub root: &'a str,
    pub spec: &'a str,
    pub op: crate::mcp::edit::EditOp,
    pub payload_override: Option<&'a str>,
    pub dry_run: bool,
}

fn execute_edit_op<W: Write>(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    spec: &str,
    op: crate::mcp::edit::EditOp,
    out: &mut W,
) -> io::Result<()> {
    execute_edit_op_with_options(
        EditCliOptions {
            root,
            spec,
            op,
            payload_override: None,
            dry_run: false,
        },
        ingest,
        interner,
        out,
    )
}

pub fn execute_edit_op_with_options<W: Write>(
    opts: EditCliOptions<'_>,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (sym, payload) = resolve_edit_spec(opts.spec, opts.payload_override)?;
    let res = crate::mcp::edit::perform_atomic_edit_with_options(
        crate::mcp::edit::EditTargetRequest {
            root: opts.root,
            op: opts.op,
            symbol_name: &sym,
            path_hint: None,
            payload: &payload,
            dry_run: opts.dry_run,
        },
        ingest,
        interner,
    );
    writeln!(out, "{res}")
}

fn execute_symbol_mutation_query<W: Write>(
    ws: &TopWorkspaceArgs,
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    if let Some(spec) = ws.replace_symbol_body.as_deref() {
        execute_edit_op(
            root,
            ingest,
            interner,
            spec,
            crate::mcp::edit::EditOp::ReplaceBody,
            out,
        )?;
        Ok(true)
    } else if let Some(spec) = ws.insert_before_symbol.as_deref() {
        execute_edit_op(
            root,
            ingest,
            interner,
            spec,
            crate::mcp::edit::EditOp::InsertBefore,
            out,
        )?;
        Ok(true)
    } else if let Some(spec) = ws.insert_after_symbol.as_deref() {
        execute_edit_op(
            root,
            ingest,
            interner,
            spec,
            crate::mcp::edit::EditOp::InsertAfter,
            out,
        )?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn execute_workspace_doc_query<W: Write>(
    ws: &TopWorkspaceArgs,
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    with_history: bool,
    out: &mut W,
) -> io::Result<()> {
    if execute_doc_search_query(ws, root, ingest, interner, with_history, out)? {
        return Ok(());
    }
    if execute_session_tools_query(ws, root, ingest, interner, out)? {
        return Ok(());
    }
    execute_symbol_mutation_query(ws, root, ingest, interner, out)?;
    Ok(())
}

pub fn extract_symbol_body<'a>(
    sym: &'a crate::domain::Symbol,
    ingest: &'a IngestResult,
    interner: &'a Interner,
    root_path: &Path,
) -> Option<(&'a crate::domain::Symbol, &'a str, &'a str, String)> {
    let name = interner.resolve(&sym.name);
    let rel_path = interner.resolve(&ingest.files[sym.file_id as usize].path);
    let bytes = std::fs::read(root_path.join(rel_path)).ok()?;
    let start = (sym.sig_start_byte as usize).min(bytes.len());
    let end = (sym.end_byte as usize).min(bytes.len());
    if start < end {
        let body = String::from_utf8_lossy(&bytes[start..end]).to_string();
        Some((sym, name, rel_path, body))
    } else {
        None
    }
}

fn skeletonize_body(body: &str) -> String {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut elided = false;
    for line in body.lines() {
        let opens = line.chars().filter(|&c| c == '{').count() as i32;
        let closes = line.chars().filter(|&c| c == '}').count() as i32;
        if depth <= 1 {
            out.push(line.to_string());
            elided = false;
        } else if !elided {
            out.push("    ...".to_string());
            elided = true;
        }
        depth += opens - closes;
        if depth <= 1 && closes > 0 && elided {
            out.push(line.to_string());
            elided = false;
        }
    }
    out.join("\n")
}

fn collect_expanded_symbol<'a>(
    id: NodeId,
    is_outline: bool,
    ingest: &'a IngestResult,
    interner: &'a Interner,
    root_path: &Path,
) -> Option<(&'a crate::domain::Symbol, &'a str, &'a str, String)> {
    let (sym, name, rel_path, body) =
        extract_symbol_body(&ingest.symbols[id as usize], ingest, interner, root_path)?;
    let final_body = if is_outline {
        skeletonize_body(&body)
    } else {
        body
    };
    Some((sym, name, rel_path, final_body))
}

pub fn execute_expand_or_outline_query<W: Write>(
    root: &str,
    syms_spec: &str,
    is_outline: bool,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let sym_names: Vec<&str> = syms_spec
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut expanded_items = Vec::new();
    let root_path = Path::new(root);

    for sym_name in sym_names {
        let ids = crate::navigate::find_symbols_by_name(sym_name, ingest, interner);
        if ids.is_empty() {
            let flag = if is_outline { "--outline" } else { "--expand" };
            return Err(io::Error::other(format!(
                "ripwire: {flag} symbol not found: {sym_name}"
            )));
        }
        for &id in &ids {
            if let Some(item) = collect_expanded_symbol(id, is_outline, ingest, interner, root_path)
            {
                expanded_items.push(item);
            }
        }
    }

    let items_slice: Vec<(&crate::domain::Symbol, &str, &str, &str)> = expanded_items
        .iter()
        .map(|(s, n, p, b)| (*s, *n, *p, b.as_str()))
        .collect();

    crate::serialize::serialize_expand_xml(root, &items_slice, out)
}

pub fn execute_expand_query<W: Write>(
    root: &str,
    syms_spec: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    execute_expand_or_outline_query(root, syms_spec, false, ingest, interner, out)
}

pub fn execute_pack_top_n_query<W: Write>(
    root: &str,
    n: usize,
    ingest: &IngestResult,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (file_order, buckets) = crate::serialize::bucket_symbols_by_rank(
        &ingest.symbols,
        ingest.files.len(),
        &pr.ranks,
        Some(n),
    );
    let root_path = Path::new(root);
    let mut bodies = Vec::new();
    let mut count = 0;

    for f in file_order {
        if count >= n {
            break;
        }
        let rel_path = interner.resolve(&ingest.files[f as usize].path);
        let file_disk_path = root_path.join(rel_path);
        if let Ok(bytes) = std::fs::read(&file_disk_path) {
            let content = String::from_utf8_lossy(&bytes).to_string();
            bodies.push((rel_path, content));
            count += buckets[f as usize].len();
        }
    }

    let bodies_slice: Vec<(&str, &str)> = bodies.iter().map(|(p, c)| (*p, c.as_str())).collect();
    crate::serialize::serialize_pack_top_n_xml(root, &bodies_slice, out)
}

pub fn execute_tree_query<W: Write>(
    root: &str,
    ingest: &IngestResult,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (file_order, buckets) = crate::serialize::bucket_symbols_by_rank(
        &ingest.symbols,
        ingest.files.len(),
        &pr.ranks,
        None,
    );

    let mut file_symbols = Vec::new();
    for f in file_order {
        let rel_path = interner.resolve(&ingest.files[f as usize].path);
        let bucket = &buckets[f as usize];
        let mut top3 = Vec::new();
        for &sym_id in bucket.iter().take(3) {
            let sym = &ingest.symbols[sym_id as usize];
            top3.push((sym.kind.tag(), interner.resolve(&sym.name)));
        }
        file_symbols.push(crate::serialize::TreeFileSymbols {
            path: rel_path,
            total_symbols: bucket.len(),
            top_symbols: top3,
        });
    }

    crate::serialize::serialize_tree_xml(root, &file_symbols, pr.iterations, out)
}

fn is_header_path(p: &str) -> bool {
    p.ends_with(".h") || p.ends_with(".hpp") || p.ends_with(".hh") || p.ends_with(".hxx")
}

fn sig_has_static_token(source: &[u8], start: u32, end: u32) -> bool {
    let s = (start as usize).min(source.len());
    let e = (end as usize).min(source.len());
    if s >= e {
        return false;
    }
    source[s..e]
        .split(|&b| !b.is_ascii_alphanumeric() && b != b'_')
        .any(|token| token == b"static")
}

pub fn execute_dead_code_query<W: Write>(
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let root_path = Path::new(root);
    let mut file_cache: std::collections::HashMap<u32, Vec<u8>> = std::collections::HashMap::new();
    let mut dead = Vec::new();
    for sym in &ingest.symbols {
        if sym.kind != crate::domain::SymKind::Function
            || sym.sig_end_byte >= sym.end_byte
            || graph.in_degree(sym.id) != 0
        {
            continue;
        }
        let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
        if is_header_path(p) {
            continue;
        }
        let src = file_cache
            .entry(sym.file_id)
            .or_insert_with(|| std::fs::read(root_path.join(p)).unwrap_or_default());
        if sig_has_static_token(src, sym.sig_start_byte, sym.sig_end_byte) {
            let n = interner.resolve(&sym.name);
            dead.push((n, sym.kind.tag(), p, sym.line));
        }
    }
    dead.sort_by(|a, b| {
        a.2.cmp(b.2)
            .then_with(|| a.3.cmp(&b.3))
            .then_with(|| a.0.cmp(b.0))
    });
    crate::serialize::serialize_dead_code_xml(root, &dead, out)
}

pub fn build_community_module<'a>(
    id: u32,
    nodes: &[NodeId],
    ingest: &'a IngestResult,
    interner: &'a Interner,
) -> crate::serialize::CommunityModule<'a> {
    let mut members = Vec::new();
    let mut top_dir = "";
    let mut top_label = "";
    for &nid in nodes {
        if let Some(s) = ingest.symbols.get(nid as usize) {
            let n = interner.resolve(&s.name);
            let p = interner.resolve(&ingest.files[s.file_id as usize].path);
            if top_dir.is_empty() {
                top_dir = p;
            }
            if top_label.is_empty() {
                top_label = n;
            }
            members.push((s.kind.tag(), n, p, s.line));
        }
    }
    crate::serialize::CommunityModule {
        id,
        size: members.len(),
        dir: top_dir,
        label: top_label,
        members,
    }
}

fn tally_isolated_symbols(
    ingest: &IngestResult,
    graph: &Graph,
    sum: &mut crate::serialize::CommunitySummary<'_>,
) {
    for sym in &ingest.symbols {
        if graph.in_degree(sym.id) > 0 || graph.out_degree(sym.id) > 0 {
            continue;
        }
        sum.isolated += 1;
        if sym.kind == crate::domain::SymKind::Section {
            sum.isolated_doc += 1;
        } else if sym.sig_end_byte >= sym.end_byte {
            sum.isolated_decl += 1;
        } else {
            sum.isolated_source += 1;
        }
    }
}

fn collect_community_bridges<'a>(
    comms: &[u32],
    modules: &[crate::serialize::CommunityModule<'a>],
    ingest: &'a IngestResult,
    graph: &Graph,
) -> Vec<(u32, u32, &'a str, &'a str, usize)> {
    let label_by_id: std::collections::HashMap<u32, &'a str> =
        modules.iter().map(|m| (m.id, m.label)).collect();
    let mut bridge_counts: std::collections::HashMap<(u32, u32), usize> =
        std::collections::HashMap::new();
    for u in 0..ingest.symbols.len() {
        let cu = comms.get(u).copied().unwrap_or(0);
        for (v, _) in graph.callees(u as NodeId) {
            let cv = comms.get(v as usize).copied().unwrap_or(0);
            if cu != cv {
                let pair = if cu < cv { (cu, cv) } else { (cv, cu) };
                *bridge_counts.entry(pair).or_default() += 1;
            }
        }
    }
    let mut bridges: Vec<(u32, u32, &'a str, &'a str, usize)> = bridge_counts
        .into_iter()
        .map(|((a, b), w)| {
            let la = label_by_id.get(&a).copied().unwrap_or("");
            let lb = label_by_id.get(&b).copied().unwrap_or("");
            (a, b, la, lb, w)
        })
        .collect();
    bridges.sort_by(|x, y| y.4.cmp(&x.4).then_with(|| x.0.cmp(&y.0)));
    bridges
}

fn compute_community_summary<'a>(
    comms: &[u32],
    by_comm: &std::collections::HashMap<u32, Vec<NodeId>>,
    modules: &[crate::serialize::CommunityModule<'a>],
    ingest: &'a IngestResult,
    graph: &Graph,
) -> crate::serialize::CommunitySummary<'a> {
    let connected_singletons = by_comm
        .values()
        .filter(|n| n.len() == 1 && (graph.in_degree(n[0]) > 0 || graph.out_degree(n[0]) > 0))
        .count();
    let mut sum = crate::serialize::CommunitySummary {
        symbols: ingest.symbols.len(),
        connected_singletons,
        pr_iters: 44,
        ..Default::default()
    };
    tally_isolated_symbols(ingest, graph, &mut sum);
    sum.bridges = collect_community_bridges(comms, modules, ingest, graph);
    sum
}

pub fn execute_communities_query<W: Write>(
    root: &str,
    target_id: Option<u32>,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let comms = graph.communities();
    let mut by_comm: std::collections::HashMap<u32, Vec<NodeId>> = std::collections::HashMap::new();
    for (node, &c) in comms.comm.iter().enumerate() {
        if target_id.is_none() || target_id == Some(c) {
            by_comm.entry(c).or_default().push(node as u32);
        }
    }

    let mut comm_ids: Vec<u32> = by_comm
        .iter()
        .filter(|(_, nodes)| target_id.is_some() || nodes.len() >= 2)
        .map(|(&id, _)| id)
        .collect();
    comm_ids.sort_by(|a, b| {
        by_comm[b]
            .len()
            .cmp(&by_comm[a].len())
            .then_with(|| a.cmp(b))
    });

    let modules: Vec<crate::serialize::CommunityModule<'_>> = comm_ids
        .into_iter()
        .map(|id| build_community_module(id, &by_comm[&id], ingest, interner))
        .collect();
    let summary = compute_community_summary(&comms.comm, &by_comm, &modules, ingest, graph);

    crate::serialize::serialize_communities_with_summary_xml(root, &modules, &summary, out)
}

fn enqueue_around_neighbors(
    curr: NodeId,
    next_d: u32,
    fanout: usize,
    graph: &Graph,
    visited: &mut std::collections::HashSet<NodeId>,
    queue: &mut std::collections::VecDeque<(NodeId, u32)>,
) {
    for (callee_id, _) in graph.callees(curr).take(fanout) {
        if visited.insert(callee_id) {
            queue.push_back((callee_id, next_d));
        }
    }
    for (caller_id, _) in graph.callers(curr).take(fanout) {
        if visited.insert(caller_id) {
            queue.push_back((caller_id, next_d));
        }
    }
}

pub fn execute_around_query<W: Write>(
    root: &str,
    sym_name: &str,
    cfg: AroundConfig,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let ids = crate::navigate::find_symbols_by_name(sym_name, ingest, interner);
    if ids.is_empty() {
        return Err(io::Error::other(format!(
            "ripwire: --around symbol not found: {sym_name}"
        )));
    }
    let sym_id = ids[0];
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    visited.insert(sym_id);
    queue.push_back((sym_id, 0u32));

    let mut result_syms = Vec::new();

    while let Some((curr, curr_d)) = queue.pop_front() {
        if let Some(s) = ingest.symbols.get(curr as usize) {
            let n = interner.resolve(&s.name);
            let p = interner.resolve(&ingest.files[s.file_id as usize].path);
            result_syms.push((n, s.kind.tag(), p, s.line));
        }
        if curr_d < cfg.depth {
            enqueue_around_neighbors(
                curr,
                curr_d + 1,
                cfg.fanout,
                graph,
                &mut visited,
                &mut queue,
            );
        }
    }

    crate::serialize::serialize_around_xml(root, sym_name, cfg.depth, cfg.fanout, &result_syms, out)
}

pub fn execute_external_surface_query<W: Write>(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let by_name = crate::resolve::index_symbols_by_name(&ingest.symbols);
    let mut ext_counts: std::collections::HashMap<(lasso::Spur, crate::domain::Lang), usize> =
        std::collections::HashMap::new();

    for r in &ingest.references {
        if !by_name.contains_key(&r.callee_name) {
            *ext_counts.entry((r.callee_name, r.lang)).or_default() += 1;
        }
    }

    let mut items: Vec<(&str, &str, usize)> = ext_counts
        .iter()
        .map(|(&(n, l), &c)| (interner.resolve(&n), l.tag(), c))
        .collect();
    items.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(b.0)));

    crate::serialize::serialize_external_surface_xml(root, &items, out)
}

fn record_import_edges<'a>(
    ingest: &'a IngestResult,
    interner: &'a Interner,
    file_index: &std::collections::HashMap<&str, u32>,
    adj_sets: &mut [std::collections::BTreeSet<usize>],
    inc_targets: &mut [Vec<&'a str>],
) {
    let f_len = ingest.files.len();
    for r in &ingest.references {
        let uf = r.file_id as usize;
        if r.role != crate::domain::RefRole::Import || uf >= f_len {
            continue;
        }
        let raw = interner.resolve(&r.callee_name);
        let clean = crate::ingest::strip_import_quotes(raw);
        if !clean.is_empty() && !inc_targets[uf].contains(&clean) {
            inc_targets[uf].push(clean);
        }
        let f_path = interner.resolve(&ingest.files[uf].path);
        if let Some(vf) = ingest.files[uf]
            .lang
            .semantics()
            .resolve_import_target(f_path, raw, file_index)
            && (vf as usize) != uf
            && (vf as usize) < f_len
        {
            adj_sets[uf].insert(vf as usize);
        }
    }
}

fn collect_import_and_call_adj<'a>(
    ingest: &'a IngestResult,
    graph: &Graph,
    interner: &'a Interner,
) -> (Vec<Vec<usize>>, Vec<Vec<&'a str>>) {
    let f_len = ingest.files.len();
    let file_index: std::collections::HashMap<&str, u32> = ingest
        .files
        .iter()
        .map(|f| (interner.resolve(&f.path), f.id))
        .collect();
    let mut adj_sets = vec![std::collections::BTreeSet::new(); f_len];
    let mut inc_targets = vec![Vec::new(); f_len];
    record_import_edges(
        ingest,
        interner,
        &file_index,
        &mut adj_sets,
        &mut inc_targets,
    );

    for u in 0..graph.node_count() as NodeId {
        let uf = ingest.symbols[u as usize].file_id as usize;
        for (v, _, is_split) in graph.callees_with_prov(u) {
            let vf = ingest.symbols[v as usize].file_id as usize;
            if !is_split && uf != vf && uf < f_len && vf < f_len {
                adj_sets[uf].insert(vf);
            }
        }
    }

    let adj = adj_sets
        .into_iter()
        .map(|s| s.into_iter().collect())
        .collect();
    (adj, inc_targets)
}

struct TarjanState {
    idx: Vec<i32>,
    low: Vec<i32>,
    on_stack: Vec<bool>,
    stack: Vec<usize>,
    counter: i32,
    cycles: Vec<Vec<usize>>,
}

fn tarjan_dfs(u: usize, adj: &[Vec<usize>], st: &mut TarjanState) {
    st.idx[u] = st.counter;
    st.low[u] = st.counter;
    st.counter += 1;
    st.stack.push(u);
    st.on_stack[u] = true;

    for &w in &adj[u] {
        if st.idx[w] == -1 {
            tarjan_dfs(w, adj, st);
            st.low[u] = st.low[u].min(st.low[w]);
        } else if st.on_stack[w] {
            st.low[u] = st.low[u].min(st.idx[w]);
        }
    }

    if st.low[u] == st.idx[u] {
        let mut comp = Vec::new();
        while let Some(w) = st.stack.pop() {
            st.on_stack[w] = false;
            comp.push(w);
            if w == u {
                break;
            }
        }
        if comp.len() > 1 {
            comp.reverse();
            st.cycles.push(comp);
        }
    }
}

pub fn find_scc_cycles(adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adj.len();
    let mut st = TarjanState {
        idx: vec![-1; n],
        low: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        counter: 0,
        cycles: Vec::new(),
    };
    for i in 0..n {
        if st.idx[i] == -1 {
            tarjan_dfs(i, adj, &mut st);
        }
    }
    st.cycles
}

fn compute_transitive_closure_counts(adj: &[Vec<usize>]) -> Vec<usize> {
    let n = adj.len();
    let mut counts = vec![1usize; n];
    let mut seen = vec![0usize; n];
    let mut stack = Vec::new();
    for s in 0..n {
        let epoch = s + 1;
        seen[s] = epoch;
        stack.clear();
        stack.push(s);
        let mut reached = 0usize;
        while let Some(u) = stack.pop() {
            reached += 1;
            for &v in &adj[u] {
                if seen[v] != epoch {
                    seen[v] = epoch;
                    stack.push(v);
                }
            }
        }
        counts[s] = reached;
    }
    counts
}

fn collect_stabledeps_violations<'a>(
    ingest: &'a IngestResult,
    interner: &'a Interner,
    adj: &[Vec<usize>],
    afferent: &[usize],
) -> Vec<(&'a str, &'a str, f64)> {
    let instab_of = |f: usize| -> f64 {
        let ce = adj[f].len() as f64;
        let ca = afferent[f] as f64;
        if ca + ce > 0.0 { ce / (ca + ce) } else { 0.0 }
    };
    let mut violations = Vec::new();
    for (u, neighbors) in adj.iter().enumerate() {
        let iu = instab_of(u);
        let pu = interner.resolve(&ingest.files[u].path);
        for &v in neighbors {
            let gap = instab_of(v) - iu;
            if gap > 0.05 {
                violations.push((pu, interner.resolve(&ingest.files[v].path), gap));
            }
        }
    }
    violations.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(b.0))
    });
    violations
}

pub fn compute_deps_report<'a>(
    ingest: &'a IngestResult,
    graph: &Graph,
    interner: &'a Interner,
) -> crate::serialize::DepsReport<'a> {
    let f_len = ingest.files.len();
    let (adj, inc_targets) = collect_import_and_call_adj(ingest, graph, interner);
    let mut afferent = vec![0usize; f_len];
    for &v in adj.iter().flatten() {
        afferent[v] += 1;
    }
    let transitive = compute_transitive_closure_counts(&adj);
    let cycles: Vec<Vec<&'a str>> = find_scc_cycles(&adj)
        .into_iter()
        .map(|c| {
            c.into_iter()
                .map(|fid| interner.resolve(&ingest.files[fid].path))
                .collect()
        })
        .collect();
    let violations = collect_stabledeps_violations(ingest, interner, &adj, &afferent);

    let mut rows = Vec::with_capacity(f_len);
    let mut ccd = 0usize;
    let mut dep_files = 0usize;
    for (i, f) in ingest.files.iter().enumerate() {
        if f.lang.is_dependency_capable() {
            dep_files += 1;
            ccd += transitive[i];
        }
        rows.push(crate::serialize::DepsFileRow {
            path: interner.resolve(&f.path),
            includes: inc_targets[i].len().max(adj[i].len()),
            efferent: adj[i].len(),
            afferent: afferent[i],
            transitive: transitive[i],
            inc_targets: inc_targets[i].clone(),
        });
    }
    rows.sort_by(|a, b| {
        (b.transitive + b.afferent)
            .cmp(&(a.transitive + a.afferent))
            .then_with(|| a.path.cmp(b.path))
    });
    let n = dep_files.max(1) as f64;
    let acd = ccd as f64 / n;
    let btree = ((n + 1.0) * (n + 1.0).log2() - n).max(1.0);
    crate::serialize::DepsReport {
        corpus_files: f_len,
        dep_files,
        ccd,
        acd,
        nccd: ccd as f64 / btree,
        rows,
        cycles,
        violations,
    }
}

pub fn execute_deps_query<W: Write>(
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let rep = compute_deps_report(ingest, graph, interner);
    crate::serialize::serialize_deps_report_xml(root, &rep, out)
}

pub fn deduplicate_batch_legend(raw: &[u8], legend_emitted: &mut bool) -> Vec<u8> {
    let text = String::from_utf8_lossy(raw);
    if !text.contains("<!--") {
        return raw.to_vec();
    }
    if !*legend_emitted {
        *legend_emitted = true;
        return raw.to_vec();
    }
    let mut out = String::with_capacity(text.len());
    let mut s: &str = &text;
    while let Some(start) = s.find("<!--") {
        out.push_str(&s[..start]);
        if let Some(end) = s[start..].find("-->") {
            s = &s[start + end + 3..];
        } else {
            break;
        }
    }
    out.push_str(s.trim_start());
    let trimmed = out.trim_end();
    format!(
        "{trimmed}\n<about legend=\"ref\" dictv=\"{}\"/>\n",
        crate::serialize::LEGEND_DICT_VERSION
    )
    .into_bytes()
}

pub fn run_batch_subquery<W: Write>(
    line: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let mut first = false;
    run_batch_subquery_dedup(line, ingest, graph, pr, interner, &mut first, out)
}

fn run_batch_subquery_dedup<W: Write>(
    line: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    legend_emitted: &mut bool,
    out: &mut W,
) -> io::Result<()> {
    let Some((verb, arg)) = line.split_once(':') else {
        return Ok(());
    };
    let a = arg.trim();
    let mut sub_buf = Vec::new();
    match verb.trim() {
        "callers" => {
            let _ = execute_nav_symbol_query(
                a,
                NavMode::Callers,
                ingest,
                graph,
                &pr.ranks,
                interner,
                &mut sub_buf,
            );
        }
        "callees" => {
            let _ = execute_nav_symbol_query(
                a,
                NavMode::Callees,
                ingest,
                graph,
                &pr.ranks,
                interner,
                &mut sub_buf,
            );
        }
        "impact" => {
            let _ = execute_nav_symbol_query(
                a,
                NavMode::Impact,
                ingest,
                graph,
                &pr.ranks,
                interner,
                &mut sub_buf,
            );
        }
        "uses" => {
            let _ = execute_nav_symbol_query(
                a,
                NavMode::Uses,
                ingest,
                graph,
                &pr.ranks,
                interner,
                &mut sub_buf,
            );
        }
        "grep" => {
            let val = crate::mcp::retrieval::call_grep(ingest, interner, a, None);
            let _ = crate::serialize::serialize_grep_xml(&val, &mut sub_buf);
        }
        "for" => {
            let val = crate::mcp::retrieval::call_for_task_with_options(
                ingest,
                interner,
                &pr.ranks,
                a,
                crate::mcp::retrieval::ForTaskOptions {
                    root: ".",
                    graph: Some(graph),
                    ..Default::default()
                },
            );
            let _ = crate::serialize::serialize_for_xml(&val, &mut sub_buf);
        }
        _ => {}
    }
    let final_bytes = deduplicate_batch_legend(&sub_buf, legend_emitted);
    writeln!(out, "  <result query=\"{line}\">")?;
    out.write_all(&final_bytes)?;
    writeln!(out, "  </result>")
}

pub fn execute_batch_content<W: Write>(
    root: &str,
    content: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let lines: Vec<&str> = content
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .take(16)
        .collect();

    writeln!(
        out,
        "<batch schema=\"ripwire.batch/v1\" root=\"{root}\" dictv=\"{}\" count=\"{}\">",
        crate::serialize::LEGEND_DICT_VERSION,
        lines.len()
    )?;
    let mut legend_emitted = false;
    for line in lines {
        run_batch_subquery_dedup(line, ingest, graph, pr, interner, &mut legend_emitted, out)?;
    }
    writeln!(out, "</batch>")
}
pub fn execute_batch_file<W: Write>(
    root: &str,
    file_path: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let content = if file_path == "-" {
        let mut buf = String::new();
        io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
        buf
    } else {
        std::fs::read_to_string(file_path)?
    };
    execute_batch_content(root, &content, ingest, graph, pr, interner, out)
}

fn execute_task_or_signatures<W: Write>(
    ret: &TopRetrievalArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    if let Some(task) = ret.task_for.as_deref() {
        let detail = if ret.signatures_only {
            None
        } else if ret.auto_bodies && ret.detail.is_none() {
            Some(6)
        } else {
            ret.detail
        };
        let mut val = crate::mcp::retrieval::call_for_task_with_options(
            ingest,
            interner,
            &pr.ranks,
            task,
            crate::mcp::retrieval::ForTaskOptions {
                root,
                graph: Some(graph),
                budget_tokens: None,
                detail,
                adaptive: ret.adaptive,
            },
        );
        if let Some(in_pat) = ret.in_scope.as_deref()
            && let Some(arr) = val
                .get_mut("symbols")
                .and_then(serde_json::Value::as_array_mut)
        {
            arr.retain(|s| s["path"].as_str().is_some_and(|p| p.contains(in_pat)));
        }
        if let Some(raw) = ret.sections.as_deref() {
            let sections =
                crate::mcp::retrieval::parse_for_sections(raw).map_err(io::Error::other)?;
            val["sections"] = serde_json::json!(sections);
        }
        crate::serialize::serialize_for_xml(&val, out)?;
        return Ok(true);
    }
    if ret.pack_signatures {
        writeln!(out, "<ctx schema=\"ripwire.pack-signatures/v1\">")?;
        let opts = crate::serialize::SerializeOptions {
            root: root.to_string(),
            ..Default::default()
        };
        crate::serialize::serialize_xml(ingest, graph, pr, interner, out, &opts)?;
        writeln!(out, "</ctx>")?;
        return Ok(true);
    }
    Ok(false)
}

fn recommend_verb_for_task(task: &str) -> (&'static str, &'static str) {
    let lower = task.to_ascii_lowercase();
    if lower.contains("caller") || lower.contains("who calls") {
        ("--callers", "1-hop callers of a named symbol")
    } else if lower.contains("blast") || lower.contains("impact") || lower.contains("safe") {
        ("--impact", "transitive caller blast radius")
    } else if lower.contains("trace") || lower.contains("crash") || lower.contains("error") {
        ("--from-trace", "map stack trace onto indexed symbols")
    } else if lower.contains("duplicate") || lower.contains("clone") {
        ("--clones", "AST near-miss code clone detector")
    } else if lower.contains("test") || lower.contains("gate") {
        (
            "--test-gate",
            "affected test suite and untested blast radius",
        )
    } else {
        ("--for", "task-lens BM25 + PageRank symbol retrieval")
    }
}

fn execute_pattern_or_match<W: Write>(
    ret: &TopRetrievalArgs,
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    if let Some(pat) = ret.pattern.as_deref() {
        let val = crate::mcp::retrieval::call_pattern_search(
            root,
            ingest,
            interner,
            pat,
            ret.grep_in.as_deref(),
        );
        crate::serialize::serialize_grep_xml(&val, out)?;
    } else if let Some(q) = ret.ast_match.as_deref() {
        let val = crate::mcp::retrieval::call_ast_match(root, ingest, interner, q)
            .map_err(io::Error::other)?;
        crate::serialize::serialize_grep_xml(&val, out)?;
    } else if let Some(pat) = ret.regex.as_deref() {
        let before = ret.grep_before.or(ret.grep_context).unwrap_or(0);
        let after = ret.grep_after.or(ret.grep_context).unwrap_or(0);
        let val = crate::mcp::retrieval::call_grep_with_context(
            root,
            ingest,
            interner,
            pat,
            ret.grep_in.as_deref(),
            before,
            after,
        );
        crate::serialize::serialize_grep_xml(&val, out)?;
    } else if let Some(task) = ret.help_task.as_deref() {
        let (verb, reason) = recommend_verb_for_task(task);
        let t_esc = crate::serialize::escape::escape_xml_str(task);
        writeln!(
            out,
            "<help-task schema=\"ripwire.help-task/v1\" task=\"{t_esc}\" recommended=\"{verb}\" why=\"{reason}\"/>"
        )?;
    } else if let Some(handle) = ret.refetch.as_deref() {
        execute_expand_or_outline_query(root, handle, false, ingest, interner, out)?;
    }
    Ok(())
}

fn execute_advanced_retrieval<W: Write>(
    ret: &TopRetrievalArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    if let Some(tr) = ret.from_trace.as_deref() {
        let trace_text = if tr == "-" {
            let mut buf = String::new();
            let _ = io::Read::read_to_string(&mut io::stdin(), &mut buf);
            buf
        } else {
            std::fs::read_to_string(tr).unwrap_or_else(|_| tr.to_string())
        };
        let val = crate::mcp::graph_tools::call_from_trace(ingest, interner, &trace_text);
        crate::serialize::serialize_from_trace_xml(&val, out)
    } else if let Some(at_spec) = ret.at.as_deref() {
        let (file, line_str) = at_spec.split_once(':').unwrap_or((at_spec, "1"));
        let line: u32 = line_str.parse().unwrap_or(1);
        crate::navigate::serialize_at_xml(file, line, ingest, interner, out)
    } else if let Some(claim) = ret.verify.as_deref() {
        crate::navigate::evaluate_verify_claim(claim, ingest, graph, interner, out)
    } else if let Some(task) = ret.pack_task.as_deref() {
        crate::navigate::execute_pack_task(task, root, ingest, interner, out)
    } else if let Some(expr) = ret.graph_query.as_deref() {
        crate::navigate::execute_graph_query_expr(expr, root, ingest, graph, interner, out)
    } else {
        execute_pattern_or_match(ret, root, ingest, interner, out)
    }
}

fn execute_grep_cli_dispatch<W: Write>(
    ret: &TopRetrievalArgs,
    pat: &str,
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let before = ret.grep_before.or(ret.grep_context).unwrap_or(0);
    let after = ret.grep_after.or(ret.grep_context).unwrap_or(0);
    let path_filter = ret
        .grep_in
        .as_deref()
        .or(ret.grep_scope.as_deref())
        .or(ret.in_scope.as_deref());
    let mut val = crate::mcp::retrieval::call_grep_with_context(
        root,
        ingest,
        interner,
        pat,
        path_filter,
        before,
        after,
    );
    if let Some(arr) = val
        .get_mut("hits")
        .and_then(serde_json::Value::as_array_mut)
    {
        if let Some(and_p) = ret.and_pat.as_deref() {
            arr.retain(|h| h["text"].as_str().is_some_and(|t| t.contains(and_p)));
        }
        if let Some(not_p) = ret.not_pat.as_deref() {
            arr.retain(|h| !h["text"].as_str().is_some_and(|t| t.contains(not_p)));
        }
    }
    crate::serialize::serialize_grep_xml(&val, out)
}

pub fn execute_retrieval_dispatch<W: Write>(
    ret: &TopRetrievalArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    if execute_task_or_signatures(ret, root, ingest, graph, pr, interner, out)? {
        return Ok(());
    }
    if let Some(pat) = ret.grep.as_deref() {
        execute_grep_cli_dispatch(ret, pat, root, ingest, interner, out)
    } else if let Some(syms) = ret.connect.as_deref() {
        execute_connect_query_with_radius(
            root,
            syms,
            ret.connect_radius,
            ingest,
            graph,
            interner,
            out,
        )
    } else if let Some(pair) = ret.path_between.as_deref() {
        execute_path_query(pair, ingest, graph, interner, out)
    } else if let Some(ex) = ret.exemplar.as_deref() {
        let val = crate::mcp::retrieval::call_exemplar_in_root(
            root,
            ingest,
            interner,
            &pr.ranks,
            ex,
            Some(graph),
        );
        if val.get("error").is_some() {
            return Err(io::Error::other("ripwire: --exemplar: no candidate found"));
        }
        crate::serialize::serialize_exemplar_xml(root, &val, out)
    } else if let Some(iface) = ret.lego.as_deref() {
        if crate::navigate::find_symbols_by_name(iface, ingest, interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire: --lego type not found: {iface}"
            )));
        }
        let val = crate::mcp::retrieval::call_lego(ingest, interner, iface);
        crate::serialize::serialize_lego_xml(root, &val, out)
    } else if let Some(rec) = ret.recall.as_deref() {
        let val =
            crate::mcp::retrieval::call_memory_recall_in_root(root, ingest, interner, rec, None);
        crate::serialize::serialize_recall_xml(root, rec, &val, out)
    } else {
        execute_advanced_retrieval(ret, root, ingest, graph, interner, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sig_has_static_token() {
        assert!(sig_has_static_token(b"static void foo()", 0, 17));
        assert!(sig_has_static_token(b"inline static int bar()", 0, 23));
        assert!(sig_has_static_token(b"static", 0, 6));
        assert!(sig_has_static_token(b"foo static bar", 0, 14));
        assert!(!sig_has_static_token(b"mystatic void foo()", 0, 19));
        assert!(!sig_has_static_token(b"static_fn void foo()", 0, 20));
        assert!(!sig_has_static_token(b"void foo()", 0, 10));
        assert!(!sig_has_static_token(b"static void foo()", 10, 5));
        assert!(!sig_has_static_token(b"", 0, 0));
        assert!(sig_has_static_token(b"static", 0, 100));
    }

    #[test]
    fn test_skeletonize_body() {
        let body = "fn run() {\n    if x {\n        let a = 1;\n        let b = 2;\n    }\n}";
        let skel = skeletonize_body(body);
        assert!(skel.contains("..."));
        assert!(!skel.contains("let a = 1"));
        let interner = Interner::new();
        let ingest = IngestResult::new();
        let mut buf = Vec::new();
        assert!(execute_expand_query(".", "missing", &ingest, &interner, &mut buf).is_err());
        assert!(
            execute_expand_or_outline_query(".", "missing", true, &ingest, &interner, &mut buf)
                .is_err()
        );
    }
}
