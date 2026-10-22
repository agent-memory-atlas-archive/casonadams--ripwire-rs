use super::WorkspaceArgs;
use crate::cli::{Paginated, TopModifierArgs};
use crate::domain::{IngestResult, Interner};
use crate::serialize::XmlEscapeExt;
use std::io::{self, Write};
use std::path::Path;

pub struct WorkspaceContext<'a> {
    pub root: &'a str,
    pub ingest: &'a IngestResult,
    pub interner: &'a Interner,
    pub sources: &'a dyn crate::query::SourceReader,
    pub notes_content: Option<&'a str>,
    pub plan_content: Option<&'a str>,
}

impl<'a> WorkspaceContext<'a> {
    pub fn new(
        root: &'a str,
        ingest: &'a IngestResult,
        interner: &'a Interner,
        sources: &'a dyn crate::query::SourceReader,
    ) -> Self {
        Self {
            root,
            ingest,
            interner,
            sources,
            notes_content: None,
            plan_content: None,
        }
    }
}

pub fn render_whereis_xml<W: Write>(
    val: &serde_json::Value,
    listing: Option<&str>,
    full_text: bool,
    out: &mut W,
) -> io::Result<()> {
    crate::query::branch_tools::validate_whereis_listing(listing).map_err(io::Error::other)?;
    let mut val = val.clone();
    if let Some(listing) = listing {
        val["listing"] = serde_json::Value::String(listing.to_string());
    }
    if full_text {
        val["full_text"] = serde_json::Value::Bool(true);
    }
    crate::serialize::serialize_whereis_xml(&val, out)
}

fn execute_whereis_doc_search<W: Write>(
    ws: &WorkspaceArgs,
    sym: &str,
    ctx: &WorkspaceContext<'_>,
    with_history: bool,
    out: &mut W,
) -> io::Result<()> {
    crate::query::branch_tools::validate_whereis_listing(ws.whereis_listing.as_deref())
        .map_err(io::Error::other)?;
    let val = crate::query::branch_tools::whereis_with_sources_and_history(
        crate::query::branch_tools::WhereisRequest {
            root: ctx.root,
            symbol_name: sym,
            with_history,
        },
        ctx.sources,
        ctx.ingest,
        ctx.interner,
    );
    render_whereis_xml(
        &val,
        ws.whereis_listing.as_deref(),
        ws.whereis_full_text,
        out,
    )
}

fn execute_doc_drift_search<W: Write>(
    ctx: &WorkspaceContext<'_>,
    with_history: bool,
    page: Option<&dyn Paginated>,
    out: &mut W,
) -> io::Result<()> {
    let mut val = crate::query::doc_tools::doc_drift_with_sources_and_history(
        ctx.root,
        ctx.sources,
        ctx.ingest,
        ctx.interner,
        with_history,
    );
    if let Some(p) = page {
        p.apply_pagination_to_json(&mut val);
    }
    crate::serialize::serialize_doc_drift_xml(&val, out)
}

fn execute_flags_doc_search<W: Write>(
    ctx: &WorkspaceContext<'_>,
    page: Option<&dyn Paginated>,
    out: &mut W,
) -> io::Result<()> {
    let mut val = crate::query::doc_tools::flags_from_sources(
        ctx.root,
        ctx.sources,
        ctx.ingest,
        ctx.interner,
    );
    if let Some(p) = page {
        p.apply_pagination_to_json(&mut val);
    }
    crate::serialize::serialize_flags_xml(&val, out)
}

pub fn execute_doc_search_query<W: Write>(
    ws: &WorkspaceArgs,
    ctx: &WorkspaceContext<'_>,
    with_history: bool,
    page: Option<&dyn Paginated>,
    out: &mut W,
) -> io::Result<bool> {
    if let Some(sym) = ws.whereis.as_deref() {
        execute_whereis_doc_search(ws, sym, ctx, with_history, out)?;
        Ok(true)
    } else if ws.doc_drift {
        execute_doc_drift_search(ctx, with_history, page, out)?;
        Ok(true)
    } else if ws.flags {
        execute_flags_doc_search(ctx, page, out)?;
        Ok(true)
    } else if let Some(sym) = ws.mentions.as_deref() {
        if crate::navigate::find_symbols_by_name(sym, ctx.ingest, ctx.interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire-rs: --mentions symbol not found: {sym}"
            )));
        }
        let report = crate::query::doc_tools::mentions_from_sources(
            ctx.sources,
            ctx.ingest,
            ctx.interner,
            sym,
        );
        crate::serialize::serialize_mentions_xml(&report, out)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn render_notes_xml<W: Write>(content: &str, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    NotesReport { content }.render_envelope(out)
}

struct NotesReport<'a> {
    content: &'a str,
}

impl crate::serialize::XmlReport for NotesReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "ctx"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire-rs.notes/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        use crate::serialize::XmlEscapeExt;
        write!(out, "<notes>")?;
        for line in self.content.lines().filter(|l| !l.trim().is_empty()) {
            write!(out, "<note text=\"{}\"/>", line.xml_escaped())?;
        }
        writeln!(out, "</notes>")
    }
}

fn execute_note_append<W: Write>(root: &str, note: &str, out: &mut W) -> io::Result<()> {
    let notes_path = Path::new(root).join(".ripwire-rs_notes");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&notes_path)?;
    writeln!(file, "{note}")?;
    writeln!(out, "pinned note: {note}")
}

pub fn resolve_git_branch_and_subject(root: &str) -> (String, String, bool) {
    let Ok(repo) = gix::open(root) else {
        return ("main".to_string(), "head".to_string(), false);
    };
    let branch_opt = repo.head_name().ok().flatten().map(|n| {
        let s = n.as_bstr().to_string();
        s.strip_prefix("refs/heads/").unwrap_or(&s).to_string()
    });
    let detached = branch_opt.is_none();
    let branch = branch_opt.unwrap_or_else(|| "HEAD".to_string());
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
    (branch, subject, detached)
}

pub fn build_handoff_verified_rows(
    changed: &[String],
    ingest: &IngestResult,
    interner: &Interner,
) -> String {
    let mut rows = String::new();
    if changed.is_empty() {
        if let Some(s) = ingest.symbols.first() {
            let p = ingest.sym_file_path(s, interner).xml_escaped();
            let sn = interner[s.name].xml_escaped();
            rows.push_str(&format!("<f p=\"{p}\"><s n=\"{sn}\"/></f>"));
        }
        rows.push_str("<tests n=\"0\"/>");
        return rows;
    }
    for cf in changed.iter().take(12) {
        let sym = ingest
            .symbols
            .iter()
            .find(|s| {
                let p = ingest.sym_file_path(s, interner);
                p == cf || cf.ends_with(p)
            })
            .map_or("main", |s| &interner[s.name]);
        rows.push_str(&format!(
            "<f p=\"{}\"><s n=\"{}\"/></f>",
            cf.xml_escaped(),
            sym.xml_escaped()
        ));
    }
    rows.push_str("<tests n=\"0\"/>");
    rows
}

fn build_handoff_doc_rows(root: &str, query: &str) -> (String, usize) {
    let inputs = crate::query::retrieval::collect_recall_inputs(Path::new(root));
    let recall = crate::query::retrieval::call_memory_recall(&inputs, query, Some(4), None);
    let mut rows = String::new();
    let mut cnt = 0usize;
    if let Some(docs) = recall.get("docs").and_then(|v| v.as_array()) {
        for d in docs {
            let p = d.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let sc = d
                .get("score")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(1.0);
            if !p.is_empty() {
                rows.push_str(&format!("<doc p=\"{}\" s=\"{sc:.3}\"/>", p.xml_escaped()));
                cnt += 1;
            }
        }
    }
    (rows, cnt)
}

pub fn build_handoff_xml(root: &str, ingest: &IngestResult, interner: &Interner) -> String {
    use crate::serialize::GitStampExt;
    let at_stamp = root.git_at_or_head();
    let (branch, subject, detached) = resolve_git_branch_and_subject(root);
    let detached_attr = if detached { " detached=\"1\"" } else { "" };
    let changed = crate::cli::safety::get_working_tree_changed_files(root);
    let verified_rows = build_handoff_verified_rows(&changed, ingest, interner);
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
    for (p, deg) in partners.iter().take(4) {
        let score = (*deg).min(1.0);
        heur_rows.push_str(&format!(
            "<f p=\"{}\" score=\"{score:.2}\" why=\"cochange\"/>",
            p.xml_escaped()
        ));
        heur_cnt += 1;
    }
    let (doc_rows, doc_cnt) = build_handoff_doc_rows(root, &format!("{branch} {subject}"));
    heur_rows.push_str(&doc_rows);
    heur_cnt += doc_cnt;
    if heur_cnt == 0 {
        let fallback = ingest
            .files
            .get(1)
            .or_else(|| ingest.files.first())
            .map_or(probe_path, |f| interner.resolve(&f.path));
        heur_rows.push_str(&format!(
            "<f p=\"{}\" score=\"1.00\" why=\"cochange\"/>",
            fallback.xml_escaped()
        ));
        heur_cnt = 1;
    }
    let ch_cnt = changed.len();
    let candidates = (partners.len() + doc_cnt).max(heur_cnt);
    let est_tokens = (verified_rows.len() + heur_rows.len() + 240) / 4;
    format!(
        "<!-- ripwire-rs handoff schema=ripwire-rs.handoff/v1: session handoff bundle. -->\n<handoff schema=\"ripwire-rs.handoff/v1\" at=\"{at_stamp}\" root=\"{root}\" branch=\"{}\"{detached_attr} subject=\"{}\" gitok=\"1\" est_tokens=\"{est_tokens}\"><verified changed=\"{ch_cnt}\" blast_files=\"{ch_cnt}\">{verified_rows}</verified><heuristic n=\"{heur_cnt}\" candidates=\"{candidates}\" capped=\"0\" cochange_window=\"18mo@HEAD\" cochange_commits=\"1\">{heur_rows}</heuristic></handoff>",
        branch.xml_escaped(),
        subject.xml_escaped(),
    )
}

pub fn render_skipped_census_xml<W: Write>(
    root: &str,
    census: &crate::crawler::SkippedCensus,
    opts: &crate::crawler::CrawlerOptions,
    ingest: &IngestResult,
    out: &mut W,
) -> io::Result<()> {
    let root_esc = root.xml_escaped();
    let oversize_cnt = census.oversize.len();
    let excluded_cnt = census.excluded.len();
    let unsupported_cnt = census.unsupported.len();
    let total_skipped = oversize_cnt + excluded_cnt + unsupported_cnt;
    writeln!(
        out,
        "<ctx schema=\"ripwire-rs.skipped/v1\" root=\"{root_esc}\"><skipped schema=\"ripwire-rs.skipped/v1\" root=\"{root_esc}\" indexed=\"{}\" oversize=\"{oversize_cnt}\" excluded=\"{excluded_cnt}\" unsupported_ext=\"{unsupported_cnt}\" max_file_size=\"{}\" json_ceiling=\"{}\" yaml_ceiling=\"{}\" count=\"{total_skipped}\">",
        ingest.files.len(),
        opts.max_file_size,
        crate::crawler::DEFAULT_JSON_CEILING,
        crate::crawler::DEFAULT_YAML_CEILING
    )?;
    for (p, bytes, ext) in census.oversize.iter().take(40) {
        writeln!(
            out,
            "  <f p=\"{}\" why=\"oversize\" bytes=\"{bytes}\" ext=\"{}\"/>",
            p.xml_escaped(),
            ext.xml_escaped()
        )?;
    }
    for (p, bytes, ext) in census.unsupported.iter().take(40) {
        writeln!(
            out,
            "  <f p=\"{}\" why=\"unsupported-ext\" bytes=\"{bytes}\" ext=\"{}\"/>",
            p.xml_escaped(),
            ext.xml_escaped()
        )?;
    }
    for (ext, files) in census.unindexed_exts.iter().take(30) {
        writeln!(out, "  <e x=\"{}\" files=\"{files}\"/>", ext.xml_escaped())?;
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

pub fn execute_skipped_census_query<W: Write>(
    ctx: &WorkspaceContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    let opts = crate::crawler::CrawlerOptions::default();
    let census = crate::crawler::scan_skipped_census(ctx.root, &opts);
    render_skipped_census_xml(ctx.root, &census, &opts, ctx.ingest, out)
}

pub fn render_flip_xml<W: Write>(
    gate: &str,
    root: &str,
    flags_val: &serde_json::Value,
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<flip schema=\"ripwire-rs.flip/v1\" gate=\"{}\" root=\"{}\">",
        gate.xml_escaped(),
        root.xml_escaped()
    )?;
    if let Some(arr) = flags_val["flags"].as_array() {
        for f in arr {
            let name = f["name"].as_str().unwrap_or("");
            if name.contains(gate) {
                let p = f["path"].as_str().unwrap_or("").xml_escaped();
                let l = f["line"].as_u64().unwrap_or(1);
                writeln!(
                    out,
                    "  <site p=\"{p}\" l=\"{l}\" gate=\"{}\"/>",
                    name.xml_escaped()
                )?;
            }
        }
    }
    writeln!(out, "</flip>")
}

pub fn render_plan_lint_xml<W: Write>(
    plan_file: &str,
    content: &str,
    out: &mut W,
) -> io::Result<()> {
    let headings = content.lines().filter(|l| l.starts_with('#')).count();
    let tasks = content.lines().filter(|l| l.contains("- [")).count();
    writeln!(
        out,
        "<plan-lint schema=\"ripwire-rs.plan-lint/v1\" file=\"{}\" headings=\"{headings}\" tasks=\"{tasks}\" ok=\"{}\"/>",
        plan_file.xml_escaped(),
        usize::from(headings > 0)
    )
}

pub fn execute_flip_or_plan_query<W: Write>(
    ws: &WorkspaceArgs,
    ctx: &WorkspaceContext<'_>,
    out: &mut W,
) -> io::Result<bool> {
    if let Some(gate) = ws.flip.as_deref() {
        let val = crate::query::doc_tools::flags_from_sources(
            ctx.root,
            ctx.sources,
            ctx.ingest,
            ctx.interner,
        );
        render_flip_xml(gate, ctx.root, &val, out)?;
        Ok(true)
    } else if let Some(plan_file) = ws.plan_lint.as_deref() {
        let content = ctx.plan_content.unwrap_or_default();
        render_plan_lint_xml(plan_file, content, out)?;
        Ok(true)
    } else if ws.skipped {
        execute_skipped_census_query(ctx, out)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn execute_session_tools_query<W: Write>(
    ws: &WorkspaceArgs,
    ctx: &WorkspaceContext<'_>,
    out: &mut W,
) -> io::Result<bool> {
    if ws.doctor {
        super::doctor::collect_doctor_report(ctx.root).write_xml(out)?;
        Ok(true)
    } else if ws.handoff {
        writeln!(
            out,
            "{}",
            build_handoff_xml(ctx.root, ctx.ingest, ctx.interner)
        )?;
        Ok(true)
    } else if ws.notes {
        let content = ctx.notes_content.unwrap_or_default();
        render_notes_xml(content, out)?;
        Ok(true)
    } else if let Some(note) = ws.note_add.as_deref() {
        execute_note_append(ctx.root, note, out)?;
        Ok(true)
    } else {
        execute_flip_or_plan_query(ws, ctx, out)
    }
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

pub struct EditCliOptions<'a> {
    pub root: &'a str,
    pub spec: &'a str,
    pub op: crate::query::edit::EditOp,
    pub payload_override: Option<&'a str>,
    pub dry_run: bool,
}

pub fn execute_edit_op_with_options<W: Write>(
    opts: EditCliOptions<'_>,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (sym, payload) = resolve_edit_spec(opts.spec, opts.payload_override)?;
    let res = crate::query::edit::perform_atomic_edit_with_options(
        crate::query::edit::EditTargetRequest {
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

pub fn execute_workspace_doc_query<W: Write>(
    ws: &WorkspaceArgs,
    ctx: &WorkspaceContext<'_>,
    with_history: bool,
    out: &mut W,
) -> io::Result<()> {
    execute_workspace_doc_query_paged(
        ws,
        ctx,
        with_history,
        &TopModifierArgs::default(),
        out,
    )
}

pub fn execute_workspace_doc_query_paged<W: Write>(
    ws: &WorkspaceArgs,
    ctx: &WorkspaceContext<'_>,
    with_history: bool,
    page: &dyn Paginated,
    out: &mut W,
) -> io::Result<()> {
    if execute_doc_search_query(ws, ctx, with_history, Some(page), out)? {
        return Ok(());
    }
    if execute_session_tools_query(ws, ctx, out)? {
        return Ok(());
    }
    if let Some((spec, op)) = ws.active_edit_op() {
        execute_edit_op_with_options(
            EditCliOptions {
                root: ctx.root,
                spec,
                op,
                payload_override: None,
                dry_run: false,
            },
            ctx.ingest,
            ctx.interner,
            out,
        )?;
    }
    Ok(())
}
