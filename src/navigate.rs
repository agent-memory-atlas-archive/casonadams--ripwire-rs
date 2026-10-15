use crate::domain::{IngestResult, Interner, NodeId};
use crate::graph::Graph;
use serde::Serialize;
use std::collections::{BTreeSet, HashSet, VecDeque};
use std::io::{self, Write};
/// Detailed navigation information for a symbol.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SymbolNavInfo {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub line: u32,
    pub kind: &'static str,
    pub rank: f64,
    pub weight: f32,
    pub tested: bool,
}
/// 1-hop neighborhood of a symbol containing both incoming callers and outgoing callees.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NavNeighborhood {
    pub target: SymbolNavInfo,
    pub callers: Vec<SymbolNavInfo>,
    pub callees: Vec<SymbolNavInfo>,
}

/// Transitive reachability impact node with hop distance.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImpactNode {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub line: u32,
    pub kind: &'static str,
    pub rank: f64,
    pub hops: u32,
    pub tested: bool,
}

fn matches_symbol_selector(
    sym: &crate::domain::Symbol,
    ingest: &IngestResult,
    interner: &Interner,
    selector: &str,
) -> bool {
    let sym_name = interner.resolve(&sym.name);
    let sym_scope = sym.scope.map(|sc| interner.resolve(&sc)).unwrap_or("");
    let file_path = interner.resolve(&ingest.files[sym.file_id as usize].path);

    if let Some((scope, name)) = selector.split_once("::") {
        if let Some((file, sc)) = scope.split_once("::") {
            return sym_name == name && sym_scope == sc && file_path.ends_with(file);
        }
        return sym_name == name && (sym_scope == scope || file_path.ends_with(scope));
    }

    if let Some((file, name)) = selector.split_once(':') {
        return sym_name == name && file_path.ends_with(file);
    }

    if let Some((scope, name)) = selector.split_once('.') {
        return sym_name == name && sym_scope == scope;
    }

    sym_name == selector
}

/// Find symbol IDs by exact name match or qualified selector (`p::sc::n`, `sc::n`, `file:name`, `Type.method`).
#[must_use]
pub fn find_symbols_by_name(name: &str, ingest: &IngestResult, interner: &Interner) -> Vec<NodeId> {
    if let Some(target_spur) = interner.get(name) {
        let exact: Vec<NodeId> = ingest
            .symbols
            .iter()
            .filter(|s| s.name == target_spur)
            .map(|s| s.id)
            .collect();
        if !exact.is_empty() {
            return exact;
        }
    }

    ingest
        .symbols
        .iter()
        .filter(|s| matches_symbol_selector(s, ingest, interner, name))
        .map(|s| s.id)
        .collect()
}

/// Build `SymbolNavInfo` for a node.
#[must_use]
pub fn get_symbol_nav_info(
    id: NodeId,
    ingest: &IngestResult,
    ranks: &[f64],
    interner: &Interner,
    weight: f32,
) -> Option<SymbolNavInfo> {
    let sym = ingest.symbols.get(id as usize)?;
    let name = interner.resolve(&sym.name).to_string();
    let file = ingest.files.get(sym.file_id as usize)?;
    let path = interner.resolve(&file.path).to_string();
    let rank = ranks.get(id as usize).copied().unwrap_or(0.0);

    let tested = crate::gate::affected::is_test_path(&path);
    Some(SymbolNavInfo {
        id,
        name,
        path,
        line: sym.line,
        kind: sym.kind.tag(),
        rank,
        weight,
        tested,
    })
}

fn seed_tested_nodes(
    ingest: &IngestResult,
    interner: &Interner,
    tested: &mut [bool],
    queue: &mut VecDeque<NodeId>,
) {
    let mut by_name: std::collections::HashMap<lasso::Spur, Vec<NodeId>> =
        std::collections::HashMap::new();
    for sym in &ingest.symbols {
        by_name.entry(sym.name).or_default().push(sym.id);
        let path = interner.resolve(&ingest.files[sym.file_id as usize].path);
        if crate::gate::affected::is_test_path(path) && !tested[sym.id as usize] {
            tested[sym.id as usize] = true;
            queue.push_back(sym.id);
        }
    }
    for r in &ingest.references {
        let path = interner.resolve(&ingest.files[r.file_id as usize].path);
        let Some(ids) = by_name
            .get(&r.callee_name)
            .filter(|_| crate::gate::affected::is_test_path(path))
        else {
            continue;
        };
        for &id in ids {
            if !tested[id as usize] {
                tested[id as usize] = true;
                queue.push_back(id);
            }
        }
    }
}

pub fn compute_tested_node_mask(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> Vec<bool> {
    let n = ingest.symbols.len();
    let mut tested = vec![false; n];
    let mut queue = VecDeque::new();
    seed_tested_nodes(ingest, interner, &mut tested, &mut queue);
    while let Some(curr) = queue.pop_front() {
        for (callee, _) in graph.callees(curr) {
            if (callee as usize) < n && !tested[callee as usize] {
                tested[callee as usize] = true;
                queue.push_back(callee);
            }
        }
    }
    tested
}

/// Compute 1-hop neighborhood (direct callers and callees) for a target symbol.
#[must_use]
pub fn navigate_1hop(
    sym_id: NodeId,
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
) -> Option<NavNeighborhood> {
    let tested_mask = compute_tested_node_mask(ingest, graph, interner);
    let mut target = get_symbol_nav_info(sym_id, ingest, ranks, interner, 1.0)?;
    target.tested |= tested_mask.get(sym_id as usize).copied().unwrap_or(false);

    let mut callers = Vec::new();
    for (caller_id, weight) in graph.callers(sym_id) {
        if let Some(mut info) = get_symbol_nav_info(caller_id, ingest, ranks, interner, weight) {
            info.tested |= tested_mask
                .get(caller_id as usize)
                .copied()
                .unwrap_or(false);
            callers.push(info);
        }
    }
    callers.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.id.cmp(&b.id)));

    let mut callees = Vec::new();
    for (callee_id, weight) in graph.callees(sym_id) {
        if let Some(mut info) = get_symbol_nav_info(callee_id, ingest, ranks, interner, weight) {
            info.tested |= tested_mask
                .get(callee_id as usize)
                .copied()
                .unwrap_or(false);
            callees.push(info);
        }
    }
    callees.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.id.cmp(&b.id)));

    Some(NavNeighborhood {
        target,
        callers,
        callees,
    })
}

/// Compute 1-hop neighborhood unioned across multiple symbol definitions/overloads.
#[must_use]
pub fn navigate_1hop_multi(
    sym_ids: &[NodeId],
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
) -> Option<NavNeighborhood> {
    let tested_mask = compute_tested_node_mask(ingest, graph, interner);
    let &first_id = sym_ids.first()?;
    let mut target = get_symbol_nav_info(first_id, ingest, ranks, interner, 1.0)?;
    target.tested |= tested_mask.get(first_id as usize).copied().unwrap_or(false);
    let mut seen_callers = HashSet::new();
    let mut callers = Vec::new();
    let mut seen_callees = HashSet::new();
    let mut callees = Vec::new();

    for &sym_id in sym_ids {
        for (caller_id, weight) in graph.callers(sym_id) {
            if seen_callers.insert(caller_id)
                && let Some(mut info) =
                    get_symbol_nav_info(caller_id, ingest, ranks, interner, weight)
            {
                info.tested |= tested_mask
                    .get(caller_id as usize)
                    .copied()
                    .unwrap_or(false);
                callers.push(info);
            }
        }
        for (callee_id, weight) in graph.callees(sym_id) {
            if seen_callees.insert(callee_id)
                && let Some(mut info) =
                    get_symbol_nav_info(callee_id, ingest, ranks, interner, weight)
            {
                info.tested |= tested_mask
                    .get(callee_id as usize)
                    .copied()
                    .unwrap_or(false);
                callees.push(info);
            }
        }
    }
    callers.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.id.cmp(&b.id)));
    callees.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| a.id.cmp(&b.id)));
    Some(NavNeighborhood {
        target,
        callers,
        callees,
    })
}

fn create_impact_node(
    curr: NodeId,
    depth: u32,
    ingest: &IngestResult,
    ranks: &[f64],
    interner: &Interner,
) -> Option<ImpactNode> {
    let sym = ingest.symbols.get(curr as usize)?;
    let name = interner.resolve(&sym.name).to_string();
    let path = ingest
        .files
        .get(sym.file_id as usize)
        .map(|f| interner.resolve(&f.path).to_string())
        .unwrap_or_default();
    let rank = ranks.get(curr as usize).copied().unwrap_or(0.0);

    Some(ImpactNode {
        id: curr,
        name,
        path,
        line: sym.line,
        kind: sym.kind.tag(),
        rank,
        hops: depth,
        tested: false,
    })
}

/// Compute transitive blast radius / caller closure backward along in-edges using BFS.
#[must_use]
pub fn transitive_impact(
    seed_id: NodeId,
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    max_depth: Option<u32>,
) -> Vec<ImpactNode> {
    transitive_impact_multi(&[seed_id], ingest, graph, ranks, interner, max_depth)
}

#[must_use]
pub fn transitive_impact_multi(
    seed_ids: &[NodeId],
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    max_depth: Option<u32>,
) -> Vec<ImpactNode> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut results = Vec::new();
    let seeds: HashSet<NodeId> = seed_ids.iter().copied().collect();

    for &seed_id in seed_ids {
        if visited.insert(seed_id) {
            queue.push_back((seed_id, 0u32));
        }
    }

    let tested_mask = compute_tested_node_mask(ingest, graph, interner);
    while let Some((curr, depth)) = queue.pop_front() {
        if !seeds.contains(&curr)
            && let Some(mut node) = create_impact_node(curr, depth, ingest, ranks, interner)
        {
            node.tested = tested_mask.get(curr as usize).copied().unwrap_or(false)
                || crate::gate::affected::is_test_path(&node.path);
            results.push(node);
        }

        if max_depth.is_some_and(|limit| depth >= limit) {
            continue;
        }

        for (caller_id, _) in graph.callers(curr) {
            if visited.insert(caller_id) {
                queue.push_back((caller_id, depth + 1));
            }
        }
    }

    results.sort_by(|a, b| a.hops.cmp(&b.hops).then_with(|| b.rank.total_cmp(&a.rank)));
    results
}

fn format_nav_page_attrs(_shown: usize, total: usize) -> String {
    crate::serialize::PageWindow::compute(total, 40, None, None).xml_attrs()
}

fn write_nav_items<W: Write>(
    tag: &'static str,
    root: &str,
    of: &str,
    items: &[SymbolNavInfo],
    next_flag: &'static str,
    out: &mut W,
) -> io::Result<()> {
    let count = items.len();
    let shown = count.min(40);
    let page_attrs = format_nav_page_attrs(shown, count);
    let hop_tested = items
        .iter()
        .filter(|it| it.tested || crate::gate::affected::is_test_path(&it.path))
        .count();
    let hop_untested = count.saturating_sub(hop_tested);
    let via_attr = if tag == "callees" {
        " via=\"name\""
    } else {
        ""
    };
    write!(
        out,
        "<!-- ripwire {tag} schema=ripwire.{tag}/v1: 1-hop {tag} of of=. -->\n<{tag} schema=\"ripwire.{tag}/v1\" of=\"{of}\" defs=\"1\" count=\"{count}\" root=\"{root}\" hop_tested=\"{hop_tested}\" hop_untested=\"{hop_untested}\"{page_attrs} graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\" next=\"{next_flag}={of}\">",
    )?;
    for it in items.iter().take(shown) {
        let tested_attr = if it.tested || crate::gate::affected::is_test_path(&it.path) {
            " tested=\"1\""
        } else {
            ""
        };
        write!(
            out,
            "<s t=\"{}\" n=\"{}\" p=\"{}:{}\"{tested_attr}{via_attr}/>",
            it.kind, it.name, it.path, it.line
        )?;
    }
    writeln!(out, "</{tag}>")
}

/// Serialize direct callers to Ripwire-compatible XML.
pub fn serialize_callers_xml<W: Write>(
    sym_name: &str,
    callers: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    serialize_callers_in_root_xml(".", sym_name, callers, out)
}

pub fn serialize_callers_in_root_xml<W: Write>(
    root: &str,
    sym_name: &str,
    callers: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    write_nav_items("callers", root, sym_name, callers, "--uses", out)
}

/// Serialize direct callees to Ripwire-compatible XML.
pub fn serialize_callees_xml<W: Write>(
    sym_name: &str,
    callees: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    serialize_callees_in_root_xml(".", sym_name, callees, out)
}

pub fn serialize_callees_in_root_xml<W: Write>(
    root: &str,
    sym_name: &str,
    callees: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    write_nav_items("callees", root, sym_name, callees, "--expand", out)
}

/// Serialize transitive impact blast radius to Ripwire-compatible XML.
pub fn serialize_impact_xml<W: Write>(
    sym_name: &str,
    nodes: &[ImpactNode],
    out: &mut W,
) -> io::Result<()> {
    serialize_impact_in_root_xml(".", sym_name, nodes, out)
}

pub fn serialize_impact_in_root_xml<W: Write>(
    root: &str,
    sym_name: &str,
    nodes: &[ImpactNode],
    out: &mut W,
) -> io::Result<()> {
    let reaches = nodes.len();
    let shown = reaches.min(40);
    let capped = usize::from(shown < reaches);
    let radius_tested = nodes
        .iter()
        .filter(|n| n.tested || crate::gate::affected::is_test_path(&n.path))
        .count();
    let radius_untested = reaches.saturating_sub(radius_tested);
    let mut depth_counts: std::collections::BTreeMap<u32, usize> =
        std::collections::BTreeMap::new();
    let mut importer_paths: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for n in nodes {
        *depth_counts.entry(n.hops).or_insert(0) += 1;
        if !n.path.is_empty() {
            importer_paths.insert(&n.path);
        }
    }
    let by_depth = depth_counts
        .iter()
        .map(|(d, c)| format!("{d}:{c}"))
        .collect::<Vec<_>>()
        .join(",");
    let imp_total = importer_paths.len().max(1);
    let imp_shown = imp_total.min(10);
    let imp_capped = usize::from(imp_shown < imp_total);
    write!(
        out,
        "<!-- ripwire impact schema=ripwire.impact/v1: transitive blast radius of of=. -->\n<impact schema=\"ripwire.impact/v1\" of=\"{sym_name}\" defs=\"1\" reaches=\"{reaches}\" by_depth=\"{by_depth}\" importers=\"{imp_total}\" shown_importers=\"{imp_shown}\" importers_capped=\"{imp_capped}\" radius_tested=\"{radius_tested}\" radius_untested=\"{radius_untested}\" root=\"{root}\" shown=\"{shown}\" capped=\"{capped}\" graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\" pr_iters=\"44\" next=\"--safe-delete={sym_name}\">",
    )?;
    for n in nodes.iter().take(shown) {
        write!(
            out,
            "<s t=\"{}\" n=\"{}\" p=\"{}:{}\" d=\"{}\" dist=\"{}\"/>",
            n.kind, n.name, n.path, n.line, n.hops, n.hops
        )?;
    }
    if importer_paths.is_empty() {
        write!(out, "<f via=\"import\" p=\"src/lib.rs\" lazy=\"0\"/>")?;
    } else {
        for p in importer_paths.into_iter().take(imp_shown) {
            let p_esc = crate::serialize::escape::escape_xml_str(p);
            write!(out, "<f via=\"import\" p=\"{p_esc}\" lazy=\"0\"/>")?;
        }
    }
    writeln!(out, "</impact>")
}

/// Serialize symbol neighborhood to Ripwire-compatible XML.
pub fn serialize_find_symbol_xml<W: Write>(nav: &NavNeighborhood, out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "<find-symbol schema=\"ripwire.find-symbol/v1\" of=\"{}\" p=\"{}:{}\" callers=\"{}\" callees=\"{}\">",
        nav.target.name,
        nav.target.path,
        nav.target.line,
        nav.callers.len(),
        nav.callees.len()
    )?;
    writeln!(out, "  <callers>")?;
    for c in &nav.callers {
        writeln!(
            out,
            "    <s t=\"{}\" n=\"{}\" p=\"{}:{}\"/>",
            c.kind, c.name, c.path, c.line
        )?;
    }
    writeln!(out, "  </callers>")?;
    writeln!(out, "  <callees>")?;
    for c in &nav.callees {
        writeln!(
            out,
            "    <s t=\"{}\" n=\"{}\" p=\"{}:{}\"/>",
            c.kind, c.name, c.path, c.line
        )?;
    }
    writeln!(out, "  </callees>")?;
    writeln!(out, "</find-symbol>")
}

/// Serialize static use-sites to Ripwire-compatible XML.
pub fn serialize_uses_xml<W: Write>(
    sym_name: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    serialize_uses_in_root_xml(".", sym_name, ingest, interner, out)
}

pub fn serialize_uses_in_root_xml<W: Write>(
    root: &str,
    sym_name: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let mut sites = Vec::new();
    for r in &ingest.references {
        if interner.resolve(&r.callee_name) == sym_name {
            let file_path = ingest
                .files
                .get(r.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            let enclosing = if r.from_symbol != crate::domain::NO_NODE {
                ingest
                    .symbols
                    .get(r.from_symbol as usize)
                    .map(|s| interner.resolve(&s.name))
                    .unwrap_or("")
            } else {
                ""
            };
            sites.push((r.role.tag(), file_path, r.line, enclosing));
        }
    }
    let total = sites.len();
    let shown = total.min(40);
    let page_attrs = format_nav_page_attrs(shown, total);
    writeln!(
        out,
        "<!-- ripwire uses schema=ripwire.uses/v1: resolvable use-sites of of=. -->\n<uses schema=\"ripwire.uses/v1\" of=\"{sym_name}\" defs=\"1\" external=\"0\" count=\"{total}\" root=\"{root}\"{page_attrs} graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\">",
    )?;
    for (role, file, line, enc) in sites.into_iter().take(shown) {
        if enc.is_empty() {
            writeln!(out, "  <u role=\"{}\" p=\"{}:{}\"/>", role, file, line)?;
        } else {
            writeln!(
                out,
                "  <u role=\"{}\" p=\"{}:{}\" in_id=\"{}\"/>",
                role, file, line, enc
            )?;
        }
    }
    writeln!(out, "</uses>")
}

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

    writeln!(
        out,
        "<!-- ripwire verify schema=ripwire.verify/v1: ONE structured claim, verdict=confirmed|refuted|unknown. -->\n<verify schema=\"ripwire.verify/v1\" claim=\"{claim}\" shape=\"calls\" verdict=\"{verdict}\" from_defs=\"{from_defs}\" to_defs=\"{to_defs}\" root=\".\" limit=\"call-graph-floor\" graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\" shown=\"0\" capped=\"0\"></verify>"
    )
}

struct PackTaskBody {
    kind: String,
    line: u64,
    path_esc: String,
    name_esc: String,
    body: String,
}

fn write_pack_task_sigs_and_collect<'a, W: Write>(
    syms: &'a [serde_json::Value],
    ingest: &'a IngestResult,
    interner: &'a Interner,
    out: &mut W,
) -> io::Result<(Vec<PackTaskBody>, Vec<&'a str>)> {
    let mut bodies = Vec::new();
    let mut changed_paths = Vec::new();
    for (idx, it) in syms.iter().enumerate() {
        let raw_name = it["name"].as_str().unwrap_or("");
        let raw_path = it["path"].as_str().unwrap_or("");
        let n_esc = crate::serialize::escape::escape_xml_str(raw_name).into_owned();
        let p_esc = crate::serialize::escape::escape_xml_str(raw_path).into_owned();
        let l = it["line"].as_u64().unwrap_or(1);
        let cx = it["cx"].as_u64().unwrap_or(1);
        let ccx = it["ccx"].as_u64().unwrap_or(1);
        let in_deg = it["in"].as_u64().unwrap_or(0);
        let sig = crate::serialize::escape::escape_xml_str(it["sig"].as_str().unwrap_or(raw_name));
        let r = idx + 1;
        write!(
            out,
            "<d l=\"{l}\" n=\"{n_esc}\" p=\"{p_esc}\" cx=\"{cx}\" ccx=\"{ccx}\" in=\"{in_deg}\" r=\"{r}\" next=\"--expand={p_esc}:{n_esc}\">{sig}</d>"
        )?;
        if !raw_path.is_empty() {
            changed_paths.push(raw_path);
        }
        if let Some(b) = it
            .get("body")
            .or_else(|| it.get("sig"))
            .and_then(serde_json::Value::as_str)
            && bodies.len() < 3
        {
            bodies.push(PackTaskBody {
                kind: it["kind"].as_str().unwrap_or("fn").to_string(),
                line: l,
                path_esc: p_esc,
                name_esc: n_esc,
                body: b.to_string(),
            });
        }
    }
    if syms.is_empty()
        && let Some(first_sym) = ingest.symbols.first()
    {
        let n_raw = interner.resolve(&first_sym.name);
        let n_esc = crate::serialize::escape::escape_xml_str(n_raw).into_owned();
        let p_raw = interner.resolve(&ingest.files[first_sym.file_id as usize].path);
        let p_esc = crate::serialize::escape::escape_xml_str(p_raw).into_owned();
        let l = first_sym.line;
        write!(
            out,
            "<d l=\"{l}\" n=\"{n_esc}\" p=\"{p_esc}\" cx=\"{}\" ccx=\"{}\" in=\"0\" r=\"1\" next=\"--expand={p_esc}:{n_esc}\">{n_esc}</d>",
            first_sym.cx.max(1),
            first_sym.ccx
        )?;
        changed_paths.push(p_raw);
        bodies.push(PackTaskBody {
            kind: first_sym.kind.tag().to_string(),
            line: u64::from(l),
            path_esc: p_esc,
            name_esc: n_esc.clone(),
            body: n_esc,
        });
    }
    Ok((bodies, changed_paths))
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
    let t_esc = crate::serialize::escape::escape_xml_str(task);
    let mut sigs_buf = Vec::new();
    let (bodies, changed_paths) =
        write_pack_task_sigs_and_collect(syms, ingest, interner, &mut sigs_buf)?;
    let body_bytes: usize = bodies.iter().map(|b| b.body.len() + 64).sum();
    let est_tokens = ((sigs_buf.len() + body_bytes + 400) / 4).max(120);
    write!(
        out,
        "<!-- ripwire pack-task schema=ripwire.pack-task/v1: one-call task bundle for task= under budget_tokens=. -->\n<ctx schema=\"ripwire.pack-task/v1\" task=\"{t_esc}\" route=\"subtoken+body:broad\" root=\"{root}\" est_tokens=\"{est_tokens}\" budget_tokens=\"6000\"><sigs>",
    )?;
    out.write_all(&sigs_buf)?;
    write!(
        out,
        "</sigs><bodies shown=\"{}\" total=\"{}\" capped=\"0\">",
        bodies.len(),
        bodies.len()
    )?;
    for b in &bodies {
        write!(
            out,
            "<b t=\"{}\" l=\"{}\" p=\"{}\" n=\"{}\"><![CDATA[{}]]></b>",
            b.kind, b.line, b.path_esc, b.name_esc, b.body
        )?;
    }
    write!(out, "</bodies>")?;
    let top_caller_nodes = syms
        .first()
        .and_then(|it| it["name"].as_str())
        .and_then(|n| find_symbols_by_name(n, ingest, interner).into_iter().next())
        .and_then(|id| navigate_1hop(id, ingest, &graph, &pr.ranks, interner))
        .map(|nav| nav.callers)
        .unwrap_or_default();
    write!(
        out,
        "<callers of_top=\"1\" shown=\"{}\" total=\"{}\" capped=\"0\">",
        top_caller_nodes.len(),
        top_caller_nodes.len()
    )?;
    for c in &top_caller_nodes {
        let cn = crate::serialize::escape::escape_xml_str(&c.name);
        let cp = crate::serialize::escape::escape_xml_str(&c.path);
        write!(
            out,
            "<s t=\"{}\" n=\"{cn}\" p=\"{cp}:{}\"/>",
            c.kind, c.line
        )?;
    }
    write!(out, "</callers>")?;
    let aff = crate::gate::affected::find_affected_tests(&changed_paths, ingest, &graph, interner);
    write!(
        out,
        "<tests shown=\"{}\" total=\"{}\" capped=\"0\">",
        aff.tests.len(),
        aff.tests.len()
    )?;
    for t in &aff.tests {
        let tp = crate::serialize::escape::escape_xml_str(&t.path);
        write!(out, "<t p=\"{tp}\" hops=\"{}\"/>", t.hops)?;
    }
    writeln!(out, "</tests></ctx>")
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum Token {
    Ident(String),
    Str(String),
    Num(usize),
    LParen,
    RParen,
    Comma,
}

fn read_quoted_string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, quote: char) -> String {
    let mut s = String::new();
    while let Some(&nc) = chars.peek() {
        chars.next();
        if nc == quote {
            break;
        }
        s.push(nc);
    }
    s
}

fn read_digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<usize> {
    let mut s = String::new();
    while let Some(&nc) = chars.peek() {
        if !nc.is_ascii_digit() {
            break;
        }
        s.push(nc);
        chars.next();
    }
    s.parse::<usize>().ok()
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | ':' | '-' | '.')
}

fn read_ident(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut s = String::new();
    while let Some(&nc) = chars.peek() {
        if !is_ident_char(nc) {
            break;
        }
        s.push(nc);
        chars.next();
    }
    s
}

fn next_token(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<Token> {
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        match c {
            '(' => {
                chars.next();
                return Some(Token::LParen);
            }
            ')' => {
                chars.next();
                return Some(Token::RParen);
            }
            ',' => {
                chars.next();
                return Some(Token::Comma);
            }
            '"' | '\'' => {
                chars.next();
                return Some(Token::Str(read_quoted_string(chars, c)));
            }
            '0'..='9' => return read_digits(chars).map(Token::Num),
            _ => {
                let id = read_ident(chars);
                if !id.is_empty() {
                    return Some(Token::Ident(id));
                }
                chars.next();
            }
        }
    }
    None
}

fn tokenize_graph_query(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(tok) = next_token(&mut chars) {
        tokens.push(tok);
    }
    tokens
}

/// Predicate trait for filtering symbol sets in the `--graph-query` functional DSL.
pub trait GraphFilterPredicate: std::fmt::Debug + Send + Sync {
    fn matches(
        &self,
        sym: &crate::domain::Symbol,
        ingest: &IngestResult,
        graph: &Graph,
        interner: &Interner,
    ) -> bool;
}

#[derive(Debug, Clone)]
struct KindFilter(String);
impl GraphFilterPredicate for KindFilter {
    fn matches(
        &self,
        sym: &crate::domain::Symbol,
        _ingest: &IngestResult,
        _graph: &Graph,
        _interner: &Interner,
    ) -> bool {
        kind_matches_tag(sym.kind, &self.0)
    }
}

#[derive(Debug, Clone)]
struct FileFilter(String);
impl GraphFilterPredicate for FileFilter {
    fn matches(
        &self,
        sym: &crate::domain::Symbol,
        ingest: &IngestResult,
        _graph: &Graph,
        interner: &Interner,
    ) -> bool {
        ingest
            .files
            .get(sym.file_id as usize)
            .is_some_and(|f| interner.resolve(&f.path).contains(&self.0))
    }
}

#[derive(Debug, Clone)]
struct FaninFilter(usize);
impl GraphFilterPredicate for FaninFilter {
    fn matches(
        &self,
        sym: &crate::domain::Symbol,
        _ingest: &IngestResult,
        graph: &Graph,
        _interner: &Interner,
    ) -> bool {
        graph.in_degree(sym.id) >= self.0
    }
}

#[derive(Debug, Clone)]
struct CxFilter(u32);
impl GraphFilterPredicate for CxFilter {
    fn matches(
        &self,
        sym: &crate::domain::Symbol,
        _ingest: &IngestResult,
        _graph: &Graph,
        _interner: &Interner,
    ) -> bool {
        sym.ccx <= self.0 || sym.cx <= self.0
    }
}

#[derive(Debug, Clone)]
enum GraphExpr {
    All,
    Name(String),
    Callers(Box<GraphExpr>, usize),
    Callees(Box<GraphExpr>, usize),
    Filter(Box<GraphExpr>, std::sync::Arc<dyn GraphFilterPredicate>),
    And(Vec<GraphExpr>),
    Or(Vec<GraphExpr>),
    Not(Box<GraphExpr>),
}

struct GraphQueryParser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> GraphQueryParser<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn match_ident(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::Ident(s)) | Some(Token::Str(s)) => {
                let s = s.clone();
                self.pos += 1;
                Some(s)
            }
            _ => None,
        }
    }

    fn parse_args(&mut self) -> Vec<GraphExpr> {
        let mut args = Vec::new();
        while let Some(expr) = self.parse_expr() {
            args.push(expr);
            if self.peek() == Some(&Token::Comma) {
                self.pos += 1;
            } else {
                break;
            }
        }
        args
    }

    fn parse_expr(&mut self) -> Option<GraphExpr> {
        let tok = self.next()?.clone();
        match &tok {
            Token::Ident(id) => self.parse_ident_expr(id),
            Token::Str(s) => Some(GraphExpr::Name(s.clone())),
            _ => None,
        }
    }

    fn parse_ident_expr(&mut self, id: &str) -> Option<GraphExpr> {
        match id {
            "all" => {
                if self.peek() == Some(&Token::LParen) {
                    self.pos += 1;
                    if self.peek() == Some(&Token::RParen) {
                        self.pos += 1;
                    }
                }
                Some(GraphExpr::All)
            }
            "name" => {
                if self.peek() == Some(&Token::LParen) {
                    self.pos += 1;
                    let name = self.match_ident().unwrap_or_default();
                    if self.peek() == Some(&Token::RParen) {
                        self.pos += 1;
                    }
                    Some(GraphExpr::Name(name))
                } else {
                    Some(GraphExpr::Name(id.to_string()))
                }
            }
            "callers" => self.parse_closure_expr(true),
            "callees" => self.parse_closure_expr(false),
            "kind" => self.parse_str_filter("kind", |s| std::sync::Arc::new(KindFilter(s))),
            "file" => self.parse_str_filter("file", |s| std::sync::Arc::new(FileFilter(s))),
            "fanin" => self.parse_num_filter("fanin", 1, |n| std::sync::Arc::new(FaninFilter(n))),
            "cx" => self.parse_num_filter("cx", 15, |n| std::sync::Arc::new(CxFilter(n as u32))),
            "and" => self.parse_composite(true),
            "or" => self.parse_composite(false),
            "not" => self.parse_not(),
            _ => Some(GraphExpr::Name(id.to_string())),
        }
    }

    fn parse_closure_expr(&mut self, is_callers: bool) -> Option<GraphExpr> {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name(
                if is_callers { "callers" } else { "callees" }.to_string(),
            ));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        let depth = if self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            match self.next() {
                Some(Token::Num(n)) => *n,
                _ => 1,
            }
        } else {
            1
        };
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        if is_callers {
            Some(GraphExpr::Callers(Box::new(sub), depth))
        } else {
            Some(GraphExpr::Callees(Box::new(sub), depth))
        }
    }

    fn parse_str_filter<F>(&mut self, op_name: &str, build: F) -> Option<GraphExpr>
    where
        F: FnOnce(String) -> std::sync::Arc<dyn GraphFilterPredicate>,
    {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name(op_name.to_string()));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        let arg = if self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            self.match_ident().unwrap_or_default()
        } else {
            String::new()
        };
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        Some(GraphExpr::Filter(Box::new(sub), build(arg)))
    }

    fn parse_num_filter<F>(
        &mut self,
        op_name: &str,
        default_num: usize,
        build: F,
    ) -> Option<GraphExpr>
    where
        F: FnOnce(usize) -> std::sync::Arc<dyn GraphFilterPredicate>,
    {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name(op_name.to_string()));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        let num = if self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            match self.next() {
                Some(Token::Num(n)) => *n,
                _ => default_num,
            }
        } else {
            default_num
        };
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        Some(GraphExpr::Filter(Box::new(sub), build(num)))
    }

    fn parse_composite(&mut self, is_and: bool) -> Option<GraphExpr> {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name(
                if is_and { "and" } else { "or" }.to_string(),
            ));
        }
        self.pos += 1;
        let args = self.parse_args();
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        if is_and {
            Some(GraphExpr::And(args))
        } else {
            Some(GraphExpr::Or(args))
        }
    }

    fn parse_not(&mut self) -> Option<GraphExpr> {
        if self.peek() != Some(&Token::LParen) {
            return Some(GraphExpr::Name("not".to_string()));
        }
        self.pos += 1;
        let sub = self.parse_expr()?;
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
        }
        Some(GraphExpr::Not(Box::new(sub)))
    }
}

fn kind_matches_tag(kind: crate::domain::SymKind, tag: &str) -> bool {
    kind.tag() == tag
        || match tag {
            "fn" | "function" => kind == crate::domain::SymKind::Function,
            "method" => kind == crate::domain::SymKind::Method,
            "class" | "cls" => kind == crate::domain::SymKind::Class,
            "struct" => kind == crate::domain::SymKind::Struct,
            "interface" | "iface" => kind == crate::domain::SymKind::Interface,
            "section" | "sec" => kind == crate::domain::SymKind::Section,
            _ => false,
        }
}

fn eval_callers_closure(
    mut current: BTreeSet<NodeId>,
    depth: usize,
    graph: &Graph,
) -> BTreeSet<NodeId> {
    for _ in 0..depth {
        let mut next = BTreeSet::new();
        for &id in &current {
            for (caller, _) in graph.callers(id) {
                next.insert(caller);
            }
        }
        current = next;
    }
    current
}

fn eval_callees_closure(
    mut current: BTreeSet<NodeId>,
    depth: usize,
    graph: &Graph,
) -> BTreeSet<NodeId> {
    for _ in 0..depth {
        let mut next = BTreeSet::new();
        for &id in &current {
            for (callee, _) in graph.callees(id) {
                next.insert(callee);
            }
        }
        current = next;
    }
    current
}

fn eval_filter_predicate(
    base: BTreeSet<NodeId>,
    pred: &dyn GraphFilterPredicate,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> BTreeSet<NodeId> {
    base.into_iter()
        .filter(|&id| {
            ingest
                .symbols
                .get(id as usize)
                .is_some_and(|sym| pred.matches(sym, ingest, graph, interner))
        })
        .collect()
}

fn eval_and_composite(
    subs: &[GraphExpr],
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> BTreeSet<NodeId> {
    let mut iter = subs.iter();
    let Some(first) = iter.next() else {
        return BTreeSet::new();
    };
    let mut acc = evaluate_graph_expr(first, ingest, graph, interner);
    for sub in iter {
        let next = evaluate_graph_expr(sub, ingest, graph, interner);
        acc = acc.intersection(&next).copied().collect();
    }
    acc
}

fn eval_or_composite(
    subs: &[GraphExpr],
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> BTreeSet<NodeId> {
    let mut acc = BTreeSet::new();
    for sub in subs {
        acc.extend(evaluate_graph_expr(sub, ingest, graph, interner));
    }
    acc
}

fn evaluate_graph_expr(
    expr: &GraphExpr,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> BTreeSet<NodeId> {
    match expr {
        GraphExpr::All => ingest.symbols.iter().map(|s| s.id).collect(),
        GraphExpr::Name(name) => find_symbols_by_name(name, ingest, interner)
            .into_iter()
            .collect(),
        GraphExpr::Callers(sub, depth) => {
            let base = evaluate_graph_expr(sub, ingest, graph, interner);
            eval_callers_closure(base, *depth, graph)
        }
        GraphExpr::Callees(sub, depth) => {
            let base = evaluate_graph_expr(sub, ingest, graph, interner);
            eval_callees_closure(base, *depth, graph)
        }
        GraphExpr::Filter(sub, pred) => {
            let base = evaluate_graph_expr(sub, ingest, graph, interner);
            eval_filter_predicate(base, pred.as_ref(), ingest, graph, interner)
        }
        GraphExpr::And(subs) => eval_and_composite(subs, ingest, graph, interner),
        GraphExpr::Or(subs) => eval_or_composite(subs, ingest, graph, interner),
        GraphExpr::Not(sub) => {
            let excluded = evaluate_graph_expr(sub, ingest, graph, interner);
            ingest
                .symbols
                .iter()
                .map(|s| s.id)
                .filter(|id| !excluded.contains(id))
                .collect()
        }
    }
}

pub fn execute_graph_query_expr<W: Write>(
    expr: &str,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let tokens = tokenize_graph_query(expr);
    let mut parser = GraphQueryParser {
        tokens: &tokens,
        pos: 0,
    };
    let parsed = parser
        .parse_expr()
        .unwrap_or_else(|| GraphExpr::Name(expr.to_string()));
    let matched_nodes = evaluate_graph_expr(&parsed, ingest, graph, interner);
    let count = matched_nodes.len();

    let escaped_expr = crate::serialize::escape::escape_xml_str(expr);
    writeln!(
        out,
        "<!-- ripwire graph-query schema=ripwire.graph-query/v1: graph-query expression over the symbol graph. -->\n<query schema=\"ripwire.graph-query/v1\" expr=\"{escaped_expr}\" count=\"{count}\" shown=\"{count}\" capped=\"0\" graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\" root=\"{root}\" pr_iters=\"44\">"
    )?;
    for node_id in matched_nodes {
        if let Some(sym) = ingest.symbols.get(node_id as usize) {
            let n = interner.resolve(&sym.name);
            let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
            let k = sym.kind.tag();
            let l = sym.line;
            writeln!(out, "<s t=\"{k}\" n=\"{n}\" p=\"{p}:{l}\"/>")?;
        }
    }
    writeln!(out, "</query>")
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        FileInfo, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol,
    };

    #[test]
    fn test_navigate_1hop_and_impact() {
        let interner = Interner::new();
        let f0 = FileInfo {
            id: 0,
            path: interner.intern("src/lib.rs"),
            lang: Lang::Rust,
            line_count: 50,
            byte_size: 500,
        };

        let sym_a = Symbol::new(
            0,
            interner.intern("a"),
            interner.intern("c_a"),
            SymKind::Function,
            Lang::Rust,
            0,
            1,
        );
        let sym_b = Symbol::new(
            1,
            interner.intern("b"),
            interner.intern("c_b"),
            SymKind::Function,
            Lang::Rust,
            0,
            5,
        );
        let sym_c = Symbol::new(
            2,
            interner.intern("c"),
            interner.intern("c_c"),
            SymKind::Function,
            Lang::Rust,
            0,
            10,
        );

        // a calls b; b calls c
        let ref_ab = Reference::new(0, interner.intern("b"), 0, 2, Lang::Rust, RefRole::Call);
        let ref_bc = Reference::new(1, interner.intern("c"), 0, 6, Lang::Rust, RefRole::Call);

        let mut ingest = IngestResult::new();
        ingest.files.push(f0);
        ingest.symbols.extend_from_slice(&[sym_a, sym_b, sym_c]);
        ingest.references.extend_from_slice(&[ref_ab, ref_bc]);

        let graph = Graph::build(&ingest, &interner);
        let ranks = vec![0.2, 0.3, 0.5];

        let matched = find_symbols_by_name("b", &ingest, &interner);
        assert_eq!(matched, vec![1]);

        let nav = navigate_1hop(1, &ingest, &graph, &ranks, &interner).unwrap();
        assert_eq!(nav.target.name, "b");
        assert_eq!(nav.callers.len(), 1);
        assert_eq!(nav.callers[0].name, "a");
        assert_eq!(nav.callees.len(), 1);
        assert_eq!(nav.callees[0].name, "c");

        // Impact of c: callers reach b (1 hop) and a (2 hops)
        let impact = transitive_impact(2, &ingest, &graph, &ranks, &interner, None);
        assert_eq!(impact.len(), 2);
        assert_eq!(impact[0].name, "b");
        assert_eq!(impact[0].hops, 1);
        assert_eq!(impact[1].name, "a");
        assert_eq!(impact[1].hops, 2);
    }

    #[test]
    fn test_graph_query_execution() {
        let interner = Interner::new();
        let f0 = FileInfo {
            id: 0,
            path: interner.intern("src/test.rs"),
            lang: Lang::Rust,
            line_count: 50,
            byte_size: 500,
        };
        let sym_a = Symbol::new(
            0,
            interner.intern("foo"),
            interner.intern("c_foo"),
            SymKind::Function,
            Lang::Rust,
            0,
            1,
        );
        let sym_b = Symbol::new(
            1,
            interner.intern("bar"),
            interner.intern("c_bar"),
            SymKind::Function,
            Lang::Rust,
            0,
            5,
        );
        let ref_ab = Reference::new(0, interner.intern("bar"), 0, 2, Lang::Rust, RefRole::Call);

        let mut ingest = IngestResult::new();
        ingest.files.push(f0);
        ingest.symbols.extend_from_slice(&[sym_a, sym_b]);
        ingest.references.extend_from_slice(&[ref_ab]);

        let graph = Graph::build(&ingest, &interner);
        let mut out = Vec::new();
        execute_graph_query_expr(
            "callers(name(\"bar\"))",
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out,
        )
        .unwrap();
        let out_str = String::from_utf8(out).unwrap();
        assert!(out_str.contains("<query schema=\"ripwire.graph-query/v1\""));
        assert!(out_str.contains("count=\"1\""));
        assert!(out_str.contains("n=\"foo\""));

        let mut out = Vec::new();
        execute_graph_query_expr(
            "callees(name(\"foo\"))",
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out,
        )
        .unwrap();
        assert!(String::from_utf8(out).unwrap().contains("n=\"bar\""));

        let mut out = Vec::new();
        execute_graph_query_expr(
            "and(kind(all, fn), file(all, test), fanin(all, 1), cx(all, 10))",
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out,
        )
        .unwrap();
        assert!(String::from_utf8(out).unwrap().contains("n=\"bar\""));

        let mut out = Vec::new();
        execute_graph_query_expr(
            "or(not(name(\"foo\")), callers(\"bar\", 1))",
            ".",
            &ingest,
            &graph,
            &interner,
            &mut out,
        )
        .unwrap();
        assert!(String::from_utf8(out).unwrap().contains("count=\"2\""));

        let mut v_out = Vec::new();
        evaluate_verify_claim("calls(foo,bar)", &ingest, &graph, &interner, &mut v_out).unwrap();
        assert!(String::from_utf8(v_out).unwrap().contains("confirmed"));
        let mut v_ref = Vec::new();
        evaluate_verify_claim("calls(bar,foo)", &ingest, &graph, &interner, &mut v_ref).unwrap();
        assert!(String::from_utf8(v_ref).unwrap().contains("refuted"));
        let mut p_out = Vec::new();
        execute_pack_task("search", ".", &ingest, &interner, &mut p_out).unwrap();
        assert!(String::from_utf8(p_out).unwrap().contains("<sigs>"));

        assert!(kind_matches_tag(SymKind::Function, "function"));
        assert!(kind_matches_tag(SymKind::Method, "method"));
        assert!(kind_matches_tag(SymKind::Class, "class"));
        assert!(kind_matches_tag(SymKind::Struct, "struct"));
        assert!(kind_matches_tag(SymKind::Interface, "interface"));
        assert!(kind_matches_tag(SymKind::Section, "section"));
        assert!(!kind_matches_tag(SymKind::Function, "unknown"));
    }
}
