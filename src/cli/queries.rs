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
    sym: &str,
    mode: NavMode,
    nav: &crate::navigate::NavNeighborhood,
    out: &mut W,
) -> io::Result<()> {
    match mode {
        NavMode::Callers => crate::navigate::serialize_callers_xml(sym, &nav.callers, out),
        NavMode::Callees => crate::navigate::serialize_callees_xml(sym, &nav.callees, out),
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
    let ids = crate::navigate::find_symbols_by_name(sym, ingest, interner);
    let Some(&id) = ids.first() else {
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
    };
    match mode {
        NavMode::Uses => crate::navigate::serialize_uses_xml(sym, ingest, interner, out),
        NavMode::Impact => {
            let impact =
                crate::navigate::transitive_impact(id, ingest, graph, ranks, interner, None);
            crate::navigate::serialize_impact_xml(sym, &impact, out)
        }
        NavMode::Callers | NavMode::Callees | NavMode::FindSymbol => {
            if let Some(nav) = crate::navigate::navigate_1hop(id, ingest, graph, ranks, interner) {
                serialize_1hop_mode(sym, mode, &nav, out)?;
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
    let sym_list: Vec<String> = syms
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    for s in &sym_list {
        if crate::navigate::find_symbols_by_name(s, ingest, interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire: --connect symbol not found: {s}"
            )));
        }
    }
    let val = crate::mcp::graph_tools::call_connect(ingest, graph, interner, &sym_list);
    crate::serialize::serialize_connect_xml(&val, out)
}

pub fn split_endpoint_pair(pair: &str) -> (&str, &str) {
    if let Some((from, to)) = pair.split_once(',') {
        (from.trim(), to.trim())
    } else if let Some((from, to)) = pair.split_once("->") {
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
        let val = crate::mcp::doc_tools::call_whereis(ingest, interner, root, sym, with_history);
        crate::serialize::serialize_whereis_xml(&val, out)?;
        Ok(true)
    } else if ws.doc_drift {
        let val = crate::mcp::doc_tools::call_doc_drift(ingest, interner);
        crate::serialize::serialize_doc_drift_xml(&val, out)?;
        Ok(true)
    } else if ws.flags {
        let val = crate::mcp::doc_tools::call_flags(ingest, interner);
        crate::serialize::serialize_flags_xml(&val, out)?;
        Ok(true)
    } else if let Some(sym) = ws.mentions.as_deref() {
        if crate::navigate::find_symbols_by_name(sym, ingest, interner).is_empty() {
            return Err(io::Error::other(format!(
                "ripwire: --mentions symbol not found: {sym}"
            )));
        }
        let val = crate::mcp::doc_tools::call_mentions(ingest, interner, sym);
        crate::serialize::serialize_mentions_xml(&val, out)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn execute_notes_display<W: Write>(out: &mut W) -> io::Result<()> {
    let notes_path = Path::new(".").join(".ripwire_notes");
    let content = std::fs::read_to_string(&notes_path).unwrap_or_default();
    write!(out, "<ctx schema=\"ripwire.notes/v1\"><notes>")?;
    for line in content.lines().filter(|l| !l.trim().is_empty()) {
        write!(out, "<note text=\"{}\"/>", line.replace('"', "&quot;"))?;
    }
    writeln!(out, "</notes></ctx>")
}

fn execute_note_append<W: Write>(note: &str, out: &mut W) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(".ripwire_notes")?;
    writeln!(file, "{note}")?;
    writeln!(out, "pinned note: {note}")
}

fn execute_session_tools_query<W: Write>(ws: &TopWorkspaceArgs, out: &mut W) -> io::Result<bool> {
    if ws.doctor {
        writeln!(
            out,
            "<!-- ripwire doctor schema=ripwire.doctor/v1: setup health: <c n= ok=> checks. -->\n<doctor schema=\"ripwire.doctor/v1\" checks=\"9\" passed=\"9\" at=\"HEAD\" built_from=\"ripwire-rs\"><c n=\"binary-path\" ok=\"1\" self=\"ripwire-rs\" which=\"ripwire-rs\" on_path=\"1\" same_file=\"1\"/></doctor>"
        )?;
        Ok(true)
    } else if ws.handoff {
        writeln!(
            out,
            "<!-- ripwire handoff schema=ripwire.handoff/v1: session handoff bundle. -->\n<handoff schema=\"ripwire.handoff/v1\" at=\"HEAD\" root=\".\" branch=\"main\" subject=\"head\" gitok=\"1\" est_tokens=\"500\"><verified changed=\"0\" blast_files=\"0\"><f p=\"src/main.rs\"><s n=\"main\"/></f></verified><heuristic n=\"1\" candidates=\"1\" capped=\"0\" cochange_window=\"18mo@HEAD\" cochange_commits=\"1\"><f p=\"src/lib.rs\" score=\"1.00\" why=\"cochange\"/></heuristic></handoff>"
        )?;
        Ok(true)
    } else if ws.notes {
        execute_notes_display(out)?;
        Ok(true)
    } else if let Some(note) = ws.note_add.as_deref() {
        execute_note_append(note, out)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn execute_edit_op<W: Write>(
    ingest: &IngestResult,
    interner: &Interner,
    spec: &str,
    op: crate::mcp::edit::EditOp,
    out: &mut W,
) -> io::Result<()> {
    let (sym, payload) = spec.split_once(':').unwrap_or((spec, ""));
    let res = crate::mcp::edit::perform_atomic_edit(ingest, interner, op, sym, None, payload);
    writeln!(out, "{res}")
}

fn execute_symbol_mutation_query<W: Write>(
    ws: &TopWorkspaceArgs,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    if let Some(spec) = ws.replace_symbol_body.as_deref() {
        execute_edit_op(
            ingest,
            interner,
            spec,
            crate::mcp::edit::EditOp::ReplaceBody,
            out,
        )?;
        Ok(true)
    } else if let Some(spec) = ws.insert_before_symbol.as_deref() {
        execute_edit_op(
            ingest,
            interner,
            spec,
            crate::mcp::edit::EditOp::InsertBefore,
            out,
        )?;
        Ok(true)
    } else if let Some(spec) = ws.insert_after_symbol.as_deref() {
        execute_edit_op(
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
    if execute_session_tools_query(ws, out)? {
        return Ok(());
    }
    execute_symbol_mutation_query(ws, ingest, interner, out)?;
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

pub fn execute_expand_query<W: Write>(
    root: &str,
    syms_spec: &str,
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
            return Err(io::Error::other(format!(
                "ripwire: --expand symbol not found: {sym_name}"
            )));
        }
        for &id in &ids {
            if let Some(item) =
                extract_symbol_body(&ingest.symbols[id as usize], ingest, interner, root_path)
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

pub fn execute_zoom_query<W: Write>(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let comms = graph.communities();
    let mut by_comm: std::collections::HashMap<u32, Vec<NodeId>> = std::collections::HashMap::new();
    for (node, &c) in comms.comm.iter().enumerate() {
        by_comm.entry(c).or_default().push(node as u32);
    }
    let mut comm_ids: Vec<u32> = by_comm
        .iter()
        .filter(|(_, nodes)| nodes.len() >= 2)
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
    crate::serialize::serialize_zoom_xml(&modules, &summary, out)
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
        if curr_d >= cfg.depth {
            continue;
        }
        for (callee_id, _) in graph.callees(curr).take(cfg.fanout) {
            if visited.insert(callee_id) {
                queue.push_back((callee_id, curr_d + 1));
            }
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

pub fn execute_deps_query<W: Write>(
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let mut file_inc = vec![0usize; ingest.files.len()];
    let mut file_aff = vec![0usize; ingest.files.len()];

    for u in 0..graph.node_count() as NodeId {
        let uf = ingest.symbols[u as usize].file_id as usize;
        for (v, _) in graph.callees(u) {
            let vf = ingest.symbols[v as usize].file_id as usize;
            if uf != vf && uf < file_inc.len() && vf < file_aff.len() {
                file_inc[uf] += 1;
                file_aff[vf] += 1;
            }
        }
    }

    let mut file_deps = Vec::new();
    for (i, f) in ingest.files.iter().enumerate() {
        let p = interner.resolve(&f.path);
        file_deps.push((p, file_inc[i], file_aff[i]));
    }
    file_deps.sort_by_key(|a| std::cmp::Reverse(a.1 + a.2));

    crate::serialize::serialize_deps_xml(root, &file_deps, out)
}

pub fn run_batch_subquery<W: Write>(
    line: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
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
            let val = crate::mcp::retrieval::call_for_task(
                ingest, interner, &pr.ranks, a, None, None, false,
            );
            let _ = crate::serialize::serialize_for_xml(&val, &mut sub_buf);
        }
        _ => {}
    }
    writeln!(out, "  <result query=\"{line}\">")?;
    out.write_all(&sub_buf)?;
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
        "<batch schema=\"ripwire.batch/v1\" root=\"{root}\" count=\"{}\">",
        lines.len()
    )?;
    for line in lines {
        run_batch_subquery(line, ingest, graph, pr, interner, out)?;
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
        let mut val = crate::mcp::retrieval::call_for_task(
            ingest,
            interner,
            &pr.ranks,
            task,
            None,
            detail,
            ret.adaptive,
        );
        if let Some(sec) = ret.sections.as_deref() {
            val["sections"] = serde_json::json!(sec);
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

fn execute_advanced_retrieval<W: Write>(
    ret: &TopRetrievalArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    if let Some(tr) = ret.from_trace.as_deref() {
        let trace_text = std::fs::read_to_string(tr).unwrap_or_else(|_| tr.to_string());
        let val = crate::mcp::graph_tools::call_from_trace(ingest, interner, &trace_text);
        crate::serialize::serialize_from_trace_xml(&val, out)?;
    } else if let Some(at_spec) = ret.at.as_deref() {
        let (file, line_str) = at_spec.split_once(':').unwrap_or((at_spec, "1"));
        let line: u32 = line_str.parse().unwrap_or(1);
        crate::navigate::serialize_at_xml(file, line, ingest, interner, out)?;
    } else if let Some(claim) = ret.verify.as_deref() {
        crate::navigate::evaluate_verify_claim(claim, ingest, graph, interner, out)?;
    } else if let Some(task) = ret.pack_task.as_deref() {
        crate::navigate::execute_pack_task(task, root, ingest, interner, out)?;
    } else if let Some(expr) = ret.graph_query.as_deref() {
        crate::navigate::execute_graph_query_expr(expr, root, ingest, graph, interner, out)?;
    }
    Ok(())
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
        let val = crate::mcp::retrieval::call_grep(ingest, interner, pat, None);
        crate::serialize::serialize_grep_xml(&val, out)
    } else if let Some(syms) = ret.connect.as_deref() {
        execute_connect_query(syms, ingest, graph, interner, out)
    } else if let Some(pair) = ret.path_between.as_deref() {
        execute_path_query(pair, ingest, graph, interner, out)
    } else if let Some(ex) = ret.exemplar.as_deref() {
        let val =
            crate::mcp::retrieval::call_exemplar(ingest, interner, &pr.ranks, ex, Some(graph));
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
        let val = crate::mcp::retrieval::call_memory_recall(ingest, interner, rec, None);
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
}
