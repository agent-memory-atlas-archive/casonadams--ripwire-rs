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
            "ripwire-rs: the at flag's seed '{file_spec}:{line_num}' named no location"
        )));
    }
    if suffix_matches.len() > 1 && !exact_match {
        return Err(io::Error::other(format!(
            "ripwire-rs: the at flag's seed '{file_spec}:{line_num}' named no location (ambiguous)"
        )));
    }
    let chain = resolve_symbol_chain_at(file_spec, line_num, ingest, interner);
    let Some(&innermost) = chain.last() else {
        return Err(io::Error::other(format!(
            "ripwire-rs: the at flag's seed '{file_spec}:{line_num}' named no location (no indexed symbol spans line {line_num})"
        )));
    };
    let name = interner.resolve(&innermost.name);
    let chain_len = chain.len();
    write!(
        out,
        "<!-- ripwire-rs at schema=ripwire-rs.at/v1: enclosing-definition chain at p=:l=. -->\n<at schema=\"ripwire-rs.at/v1\" p=\"{file_spec}\" l=\"{line_num}\" sym=\"{name}\" chain=\"{chain_len}\" root=\".\">",
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

struct PackRenderEnv<'a> {
    task: &'a str,
    root: &'a str,
    ingest: &'a IngestResult,
    graph: &'a Graph,
    ranks: &'a [f64],
    interner: &'a Interner,
}

fn collect_pack_surface_and_callers(
    env: &PackRenderEnv<'_>,
    syms: &[serde_json::Value],
) -> (
    Vec<super::SymbolNavInfo>,
    std::collections::BTreeSet<crate::domain::NodeId>,
) {
    let top_caller_nodes = syms
        .first()
        .and_then(|it| it["name"].as_str())
        .and_then(|n| {
            find_symbols_by_name(n, env.ingest, env.interner)
                .into_iter()
                .next()
        })
        .and_then(|id| navigate_1hop(id, env.ingest, env.graph, env.ranks, env.interner))
        .map(|nav| nav.callers)
        .unwrap_or_default();
    let mut surface = std::collections::BTreeSet::new();
    for it in syms {
        if let Some(n) = it["name"].as_str() {
            surface.extend(find_symbols_by_name(n, env.ingest, env.interner));
        }
    }
    for c in top_caller_nodes.iter().take(MAX_PACK_CALLERS) {
        surface.insert(c.id);
    }
    (top_caller_nodes, surface)
}

fn write_pack_task_ctx<W: Write>(
    env: &PackRenderEnv<'_>,
    syms: &[serde_json::Value],
    budget_tokens: usize,
    include_legend: bool,
    out: &mut W,
) -> io::Result<std::collections::BTreeSet<crate::domain::NodeId>> {
    let mut sigs_buf = Vec::new();
    let (bodies, changed_paths, total_bodies) =
        write_pack_task_sigs_and_collect(syms, env.ingest, env.interner, &mut sigs_buf)?;
    let (top_caller_nodes, surface) = collect_pack_surface_and_callers(env, syms);
    let aff = crate::gate::affected::find_affected_tests(
        &changed_paths,
        env.ingest,
        env.graph,
        env.interner,
    );
    let callers_shown = top_caller_nodes.len().min(MAX_PACK_CALLERS);
    let callers_capped = usize::from(callers_shown < top_caller_nodes.len());
    let tests_shown = aff.tests.len().min(MAX_PACK_TESTS);
    let tests_capped = usize::from(tests_shown < aff.tests.len());
    let bodies_capped = usize::from(bodies.len() < total_bodies);
    let body_bytes: usize = bodies.iter().map(|b| b.body.len() + 64).sum();
    let aux_bytes = callers_shown * 56 + tests_shown * 64 + 320;
    let est_tokens = ((sigs_buf.len() + body_bytes + aux_bytes) / 4).max(32);
    if include_legend {
        writeln!(
            out,
            "<!-- ripwire-rs pack-task schema=ripwire-rs.pack-task/v1: one-call task bundle for task= under budget_tokens=. -->"
        )?;
    }
    write!(
        out,
        "<ctx schema=\"ripwire-rs.pack-task/v1\" task=\"{}\" route=\"subtoken+body:broad\" root=\"{}\" est_tokens=\"{est_tokens}\" budget_tokens=\"{budget_tokens}\"><sigs>",
        env.task.xml_escaped(),
        env.root
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
    write!(
        out,
        "</bodies><callers of_top=\"1\" shown=\"{callers_shown}\" total=\"{}\" capped=\"{callers_capped}\">",
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
    write!(
        out,
        "</callers><tests shown=\"{tests_shown}\" total=\"{}\" capped=\"{tests_capped}\">",
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
    write!(out, "</tests></ctx>")?;
    if include_legend {
        writeln!(out)?;
    }
    Ok(surface)
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
    let env = PackRenderEnv {
        task,
        root,
        ingest,
        graph: &graph,
        ranks: &pr.ranks,
        interner,
    };
    let _ = write_pack_task_ctx(&env, syms, 6000, true, out)?;
    Ok(())
}

fn build_masked_node_json(
    id: crate::domain::NodeId,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> Option<serde_json::Value> {
    let sym = ingest.sym(id)?;
    let name = interner.resolve(&sym.name);
    let path = ingest.sym_file_path(sym, interner);
    let bytes = crate::domain::read_rel_bytes(std::path::Path::new(root), path).ok();
    let sig = bytes
        .as_deref()
        .map_or_else(|| name.to_string(), |b| sym.extract_signature(b, name));
    let body = bytes
        .as_deref()
        .and_then(|b| sym.source_text(b).map(std::borrow::Cow::into_owned))
        .unwrap_or_else(|| sig.clone());
    Some(serde_json::json!({
        "name": name,
        "path": path,
        "line": sym.line,
        "kind": sym.kind.tag(),
        "cx": sym.cx.max(1),
        "ccx": sym.ccx,
        "in": graph.in_degree(id),
        "sig": sig,
        "body": body,
    }))
}

fn render_masked_pack_bundle(
    env: &PackRenderEnv<'_>,
    keep_ids: &[crate::domain::NodeId],
    budget_tokens: usize,
) -> io::Result<(String, std::collections::BTreeSet<crate::domain::NodeId>)> {
    let syms: Vec<serde_json::Value> = keep_ids
        .iter()
        .filter_map(|&id| build_masked_node_json(id, env.root, env.ingest, env.graph, env.interner))
        .collect();
    let mut buf = Vec::new();
    let surface = write_pack_task_ctx(env, &syms, budget_tokens, false, &mut buf)?;
    Ok((String::from_utf8_lossy(&buf).into_owned(), surface))
}

pub fn execute_pack_task_partitioned<W: Write>(
    task: &str,
    root: &str,
    requested: usize,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let n = requested.clamp(2, 16);
    let pr = graph.page_rank(&crate::rank::PageRankConfig::default());
    let plan = crate::gate::plan_task_partition(task, n, ingest, graph, &pr.ranks, interner);
    let core_budget = 2040usize;
    let part_budget = 3960usize;
    let env = PackRenderEnv {
        task,
        root,
        ingest,
        graph,
        ranks: &pr.ranks,
        interner,
    };
    let (core_xml, core_surface) = render_masked_pack_bundle(&env, &plan.core_ids, core_budget)?;
    let mut part_xmls = Vec::with_capacity(plan.groups.len());
    let mut part_surfaces = Vec::with_capacity(plan.groups.len());
    for grp in &plan.groups {
        let (pxml, psurf) = render_masked_pack_bundle(&env, grp, part_budget)?;
        part_xmls.push(pxml);
        part_surfaces.push(psurf);
    }
    let ov = crate::gate::measure_partition_overlap(&part_surfaces, &core_surface);
    let total_bytes: usize = core_xml.len() + part_xmls.iter().map(String::len).sum::<usize>();
    write!(
        out,
        "<!-- ripwire-rs partitioned task bundle: ONE shared core plus N minimally overlapping slices along call-graph communities. -->\n<ctx-partitions schema=\"ripwire-rs.pack-task/v1\" partitions=\"{}\" requested=\"{n}\" core_symbols=\"{}\" surface=\"{}\" modules=\"{}\" split=\"{}\" budget_per_agent_tokens=\"6000\" core_budget_tokens=\"{core_budget}\" partition_budget_tokens=\"{part_budget}\" total_bytes=\"{total_bytes}\" overlap_mean=\"{:.3}\" overlap_max=\"{:.3}\" shared_symbols=\"{}\" union_symbols=\"{}\" core_overlap=\"{:.3}\">",
        part_xmls.len(),
        plan.core_ids.len(),
        plan.surface_count,
        plan.module_count,
        plan.split_count,
        ov.mean,
        ov.worst,
        ov.shared_count,
        ov.union_count,
        ov.core_leak,
    )?;
    let core_toks = (core_xml.len() * 100 / 236).max(1);
    write!(
        out,
        "<bundle role=\"core\" symbols=\"{}\" bytes=\"{}\" tokens=\"{core_toks}\" est_tokens=\"{core_toks}\">{core_xml}</bundle>",
        plan.core_ids.len(),
        core_xml.len()
    )?;
    for (idx, pxml) in part_xmls.iter().enumerate() {
        let syms_cnt = plan.groups.get(idx).map_or(0, Vec::len);
        let mods_cnt = plan.group_module_count.get(idx).copied().unwrap_or(1);
        let p_toks = (pxml.len() * 100 / 236).max(1);
        write!(
            out,
            "<bundle role=\"partition\" i=\"{idx}\" symbols=\"{syms_cnt}\" modules=\"{mods_cnt}\" bytes=\"{}\" tokens=\"{p_toks}\" est_tokens=\"{p_toks}\">{pxml}</bundle>",
            pxml.len()
        )?;
    }
    writeln!(out, "</ctx-partitions>")
}
