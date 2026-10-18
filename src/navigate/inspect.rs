use super::{find_symbols_by_name, navigate_1hop};
use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::serialize::XmlEscapeExt;
use std::io::{self, Write};

#[inline]
fn symbol_spans_line(sym: &crate::domain::Symbol, line_num: u32) -> bool {
    sym.line <= line_num && (sym.line + sym.loc) >= line_num
}

pub fn resolve_symbol_chain_at<'a>(
    file_spec: &str,
    line_num: u32,
    ingest: &'a IngestResult,
    interner: &Interner,
) -> Vec<&'a crate::domain::Symbol> {
    let mut chain: Vec<&'a crate::domain::Symbol> = ingest
        .symbols
        .iter()
        .filter(|sym| {
            let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
            (p == file_spec || p.ends_with(file_spec)) && symbol_spans_line(sym, line_num)
        })
        .collect();
    chain.sort_by_key(|s| (s.line, std::cmp::Reverse(s.loc)));
    chain
}

pub fn resolve_symbol_at<'a>(
    file_spec: &str,
    line_num: u32,
    ingest: &'a IngestResult,
    interner: &Interner,
) -> Option<&'a crate::domain::Symbol> {
    resolve_symbol_chain_at(file_spec, line_num, ingest, interner)
        .into_iter()
        .last()
}

pub fn serialize_at_xml<W: Write>(
    file_spec: &str,
    line_num: u32,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let exact_match = ingest
        .files
        .iter()
        .any(|f| interner.resolve(&f.path) == file_spec);
    let suffix_matches: Vec<_> = ingest
        .files
        .iter()
        .filter(|f| {
            let p = interner.resolve(&f.path);
            p == file_spec || p.ends_with(file_spec)
        })
        .collect();

    if suffix_matches.is_empty() {
        return Err(io::Error::other(format!(
            "ripwire: the at flag's seed '{file_spec}:{line_num}' named no location"
        )));
    }
    if suffix_matches.len() > 1 && !exact_match {
        return Err(io::Error::other(format!(
            "ripwire: the at flag's seed '{file_spec}:{line_num}' named no location (ambiguous)"
        )));
    }
    let chain = resolve_symbol_chain_at(file_spec, line_num, ingest, interner);
    let Some(&innermost) = chain.last() else {
        return Err(io::Error::other(format!(
            "ripwire: the at flag's seed '{file_spec}:{line_num}' named no location (no indexed symbol spans line {line_num})"
        )));
    };
    let name = interner.resolve(&innermost.name);
    let chain_len = chain.len();
    write!(
        out,
        "<!-- ripwire at schema=ripwire.at/v1: enclosing-definition chain at p=:l=. -->\n<at schema=\"ripwire.at/v1\" p=\"{file_spec}\" l=\"{line_num}\" sym=\"{name}\" chain=\"{chain_len}\" root=\".\">",
    )?;
    for sym in chain {
        let sn = interner.resolve(&sym.name);
        let kind = sym.kind.tag();
        let end_l = sym.line.saturating_add(sym.loc.saturating_sub(1));
        write!(
            out,
            "<s n=\"{sn}\" t=\"{kind}\" l=\"{}\" el=\"{end_l}\"/>",
            sym.line
        )?;
    }
    writeln!(out, "</at>")
}

fn check_call_relationship(
    caller_name: &str,
    callee_name: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> bool {
    for sym in &ingest.symbols {
        if interner.resolve(&sym.name) != caller_name {
            continue;
        }
        for (callee_id, _) in graph.callees(sym.id) {
            let Some(c) = ingest.symbols.get(callee_id as usize) else {
                continue;
            };
            if interner.resolve(&c.name) == callee_name {
                return true;
            }
        }
    }
    false
}

pub struct VerifyReport<'a> {
    pub claim: &'a str,
    pub verdict: &'static str,
    pub from_defs: usize,
    pub to_defs: usize,
    pub gauges: crate::serialize::GraphGauges,
}

impl crate::serialize::XmlReport for VerifyReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "verify"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.verify/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire verify schema=ripwire.verify/v1: ONE structured claim, verdict=confirmed|refuted|unknown.",
        )
    }
    fn graph_gauges(&self) -> Option<crate::serialize::GraphGauges> {
        Some(self.gauges)
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        write!(
            out,
            " claim=\"{}\" shape=\"calls\" verdict=\"{}\" from_defs=\"{}\" to_defs=\"{}\" limit=\"call-graph-floor\" shown=\"0\" capped=\"0\"",
            self.claim, self.verdict, self.from_defs, self.to_defs
        )
    }
    fn write_body<W: Write>(&self, _out: &mut W) -> io::Result<()> {
        Ok(())
    }
}

pub fn evaluate_verify_claim<W: Write>(
    claim: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let mut from_defs = 1usize;
    let mut to_defs = 1usize;
    let verdict = if claim.starts_with("calls(") && claim.ends_with(')') {
        let inner = &claim[6..claim.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() == 2 {
            let caller_name = parts[0];
            let callee_name = parts[1];
            from_defs = find_symbols_by_name(caller_name, ingest, interner).len();
            to_defs = find_symbols_by_name(callee_name, ingest, interner).len();
            if from_defs > 1 {
                "not-established"
            } else if check_call_relationship(caller_name, callee_name, ingest, graph, interner) {
                "confirmed"
            } else {
                "refuted"
            }
        } else {
            "not-established"
        }
    } else {
        "confirmed"
    };
    use crate::serialize::XmlReport;
    VerifyReport {
        claim,
        verdict,
        from_defs,
        to_defs,
        gauges: graph.gauges(),
    }
    .render_envelope(out)
}

struct PackTaskBody {
    kind: String,
    line: u64,
    path_esc: String,
    name_esc: String,
    body: String,
}

const MAX_PACK_BODY_BYTES: usize = 3600;
const MAX_PACK_TOTAL_BODY_BYTES: usize = 9600;
const MAX_PACK_CALLERS: usize = 12;
const MAX_PACK_TESTS: usize = 20;

pub fn truncate_body_to_budget(raw: &str, max_bytes: usize) -> String {
    if raw.len() <= max_bytes {
        return raw.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !raw.is_char_boundary(end) {
        end -= 1;
    }
    if let Some(nl) = raw[..end].rfind('\n') {
        end = nl;
    }
    format!(
        "{}\n// ... [truncated to fit pack-task budget]",
        &raw[..end]
    )
}

fn maybe_push_pack_body(
    it: &serde_json::Value,
    l: u64,
    p_esc: String,
    n_esc: String,
    bodies: &mut Vec<PackTaskBody>,
    acc_body_bytes: &mut usize,
) -> bool {
    let Some(b) = it
        .get("body")
        .or_else(|| it.get("sig"))
        .and_then(serde_json::Value::as_str)
    else {
        return false;
    };
    let remaining = MAX_PACK_TOTAL_BODY_BYTES.saturating_sub(*acc_body_bytes);
    if bodies.len() < 3 && remaining >= 256 {
        let capped_body = truncate_body_to_budget(b, MAX_PACK_BODY_BYTES.min(remaining));
        *acc_body_bytes += capped_body.len();
        bodies.push(PackTaskBody {
            kind: it["kind"].as_str().unwrap_or("fn").to_string(),
            line: l,
            path_esc: p_esc,
            name_esc: n_esc,
            body: capped_body,
        });
    }
    true
}

fn write_pack_task_sigs_and_collect<'a, W: Write>(
    syms: &'a [serde_json::Value],
    ingest: &'a IngestResult,
    interner: &'a Interner,
    out: &mut W,
) -> io::Result<(Vec<PackTaskBody>, Vec<&'a str>, usize)> {
    let mut bodies = Vec::new();
    let mut changed_paths = Vec::new();
    let mut total_body_candidates = 0usize;
    let mut acc_body_bytes = 0usize;
    for (idx, it) in syms.iter().enumerate() {
        let raw_name = it["name"].as_str().unwrap_or("");
        let raw_path = it["path"].as_str().unwrap_or("");
        let n_esc = raw_name.xml_escaped().to_string();
        let p_esc = raw_path.xml_escaped().to_string();
        let l = it["line"].as_u64().unwrap_or(1);
        let cx = it["cx"].as_u64().unwrap_or(1);
        let ccx = it["ccx"].as_u64().unwrap_or(1);
        let in_deg = it["in"].as_u64().unwrap_or(0);
        let sig = it["sig"].as_str().unwrap_or(raw_name).xml_escaped();
        let r = idx + 1;
        write!(
            out,
            "<d l=\"{l}\" n=\"{n_esc}\" p=\"{p_esc}\" cx=\"{cx}\" ccx=\"{ccx}\" in=\"{in_deg}\" r=\"{r}\" next=\"--expand={p_esc}:{n_esc}\">{sig}</d>"
        )?;
        if !raw_path.is_empty() {
            changed_paths.push(raw_path);
        }
        if maybe_push_pack_body(it, l, p_esc, n_esc, &mut bodies, &mut acc_body_bytes) {
            total_body_candidates += 1;
        }
    }
    if syms.is_empty()
        && let Some(first_sym) = ingest.symbols.first()
    {
        let n_raw = &interner[first_sym.name];
        let n_esc = n_raw.xml_escaped().to_string();
        let p_raw = ingest.sym_file_path(first_sym, interner);
        let p_esc = p_raw.xml_escaped().to_string();
        let l = first_sym.line;
        write!(
            out,
            "<d l=\"{l}\" n=\"{n_esc}\" p=\"{p_esc}\" cx=\"{}\" ccx=\"{}\" in=\"0\" r=\"1\" next=\"--expand={p_esc}:{n_esc}\">{n_esc}</d>",
            first_sym.cx.max(1),
            first_sym.ccx
        )?;
        changed_paths.push(p_raw);
        total_body_candidates = 1;
        bodies.push(PackTaskBody {
            kind: first_sym.kind.tag().to_string(),
            line: u64::from(l),
            path_esc: p_esc,
            name_esc: n_esc.clone(),
            body: n_esc,
        });
    }
    Ok((bodies, changed_paths, total_body_candidates))
}

pub fn execute_pack_task<W: Write>(
    task: &str,
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let graph = Graph::build(ingest, interner);
    let pr = graph.page_rank(&crate::rank::PageRankConfig::default());
    let for_val = crate::mcp::retrieval::call_for_task_with_options(
        ingest,
        interner,
        &pr.ranks,
        task,
        crate::mcp::retrieval::ForTaskOptions {
            root,
            graph: Some(&graph),
            budget_tokens: Some(10),
            detail: Some(3),
            adaptive: false,
        },
    );
    let empty = Vec::new();
    let syms = for_val["symbols"].as_array().unwrap_or(&empty);
    let mut sigs_buf = Vec::new();
    let (bodies, changed_paths, total_bodies) =
        write_pack_task_sigs_and_collect(syms, ingest, interner, &mut sigs_buf)?;
    let top_caller_nodes = syms
        .first()
        .and_then(|it| it["name"].as_str())
        .and_then(|n| find_symbols_by_name(n, ingest, interner).into_iter().next())
        .and_then(|id| navigate_1hop(id, ingest, &graph, &pr.ranks, interner))
        .map(|nav| nav.callers)
        .unwrap_or_default();
    let aff = crate::gate::affected::find_affected_tests(&changed_paths, ingest, &graph, interner);
    let callers_shown = top_caller_nodes.len().min(MAX_PACK_CALLERS);
    let callers_capped = usize::from(callers_shown < top_caller_nodes.len());
    let tests_shown = aff.tests.len().min(MAX_PACK_TESTS);
    let tests_capped = usize::from(tests_shown < aff.tests.len());
    let bodies_capped = usize::from(bodies.len() < total_bodies);
    let body_bytes: usize = bodies.iter().map(|b| b.body.len() + 64).sum();
    let aux_bytes = callers_shown * 56 + tests_shown * 64 + 400;
    let est_tokens = ((sigs_buf.len() + body_bytes + aux_bytes) / 4).max(120);
    write!(
        out,
        "<!-- ripwire pack-task schema=ripwire.pack-task/v1: one-call task bundle for task= under budget_tokens=. -->\n<ctx schema=\"ripwire.pack-task/v1\" task=\"{}\" route=\"subtoken+body:broad\" root=\"{root}\" est_tokens=\"{est_tokens}\" budget_tokens=\"6000\"><sigs>",
        task.xml_escaped()
    )?;
    out.write_all(&sigs_buf)?;
    write!(
        out,
        "</sigs><bodies shown=\"{}\" total=\"{}\" capped=\"{bodies_capped}\">",
        bodies.len(),
        total_bodies.max(bodies.len())
    )?;
    for b in &bodies {
        write!(
            out,
            "<b t=\"{}\" l=\"{}\" p=\"{}\" n=\"{}\"><![CDATA[{}]]></b>",
            b.kind, b.line, b.path_esc, b.name_esc, b.body
        )?;
    }
    write!(out, "</bodies>")?;
    write!(
        out,
        "<callers of_top=\"1\" shown=\"{callers_shown}\" total=\"{}\" capped=\"{callers_capped}\">",
        top_caller_nodes.len()
    )?;
    for c in top_caller_nodes.iter().take(callers_shown) {
        write!(
            out,
            "<s t=\"{}\" n=\"{}\" p=\"{}:{}\"/>",
            c.kind,
            c.name.xml_escaped(),
            c.path.xml_escaped(),
            c.line
        )?;
    }
    write!(out, "</callers>")?;
    write!(
        out,
        "<tests shown=\"{tests_shown}\" total=\"{}\" capped=\"{tests_capped}\">",
        aff.tests.len()
    )?;
    for t in aff.tests.iter().take(tests_shown) {
        write!(
            out,
            "<t p=\"{}\" hops=\"{}\"/>",
            t.path.xml_escaped(),
            t.hops
        )?;
    }
    writeln!(out, "</tests></ctx>")
}
