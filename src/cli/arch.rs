use super::args::AroundConfig;
use crate::domain::{IngestResult, Interner, NodeId};
use crate::graph::Graph;
use std::io::{self, Write};

fn is_header_path(p: &str) -> bool {
    p.ends_with(".h") || p.ends_with(".hpp") || p.ends_with(".hh") || p.ends_with(".hxx")
}

pub fn sig_has_static_token(source: &[u8], start: u32, end: u32) -> bool {
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
    execute_dead_code_query_paged(
        root,
        ingest,
        graph,
        interner,
        &super::args::TopModifierArgs::default(),
        out,
    )
}

pub fn execute_dead_code_query_paged<W: Write>(
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    page: &dyn super::args::Paginated,
    out: &mut W,
) -> io::Result<()> {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let mut file_cache: std::collections::HashMap<u32, Vec<u8>> = std::collections::HashMap::new();
    let mut dead = Vec::new();
    for sym in &ingest.symbols {
        if sym.kind != crate::domain::SymKind::Function
            || sym.sig_end_byte >= sym.end_byte
            || graph.in_degree(sym.id) != 0
        {
            continue;
        }
        let p = view.sym_path(sym);
        if is_header_path(p) {
            continue;
        }
        let src = file_cache
            .entry(sym.file_id)
            .or_insert_with(|| crate::domain::read_rel_bytes(root, p).unwrap_or_default());
        if sig_has_static_token(src, sym.sig_start_byte, sym.sig_end_byte) {
            dead.push((view.sym_name(sym), sym.kind.tag(), p, sym.line));
        }
    }
    dead.sort_by(|a, b| {
        a.2.cmp(b.2)
            .then_with(|| a.3.cmp(&b.3))
            .then_with(|| a.0.cmp(b.0))
    });
    crate::serialize::query_xml::serialize_dead_code_paged_xml(
        root,
        &dead,
        graph.gauges(),
        page.limit_opt(),
        page.offset_opt(),
        out,
    )
}

fn community_anchor_priority<'a, V: crate::domain::CorpusView<'a> + ?Sized>(
    view: &V,
    s: &crate::domain::Symbol,
) -> (u8, u32) {
    let prod = view.is_production_symbol(s);
    let tier = if prod && (s.is_callable() || s.kind.is_type_container()) {
        3
    } else if prod {
        2
    } else if s.is_code_symbol() {
        1
    } else {
        0
    };
    (tier, s.loc.saturating_add(s.ccx.saturating_mul(2)))
}

pub fn build_community_module<'a>(
    id: u32,
    nodes: &[NodeId],
    ingest: &'a IngestResult,
    interner: &'a Interner,
) -> crate::serialize::CommunityModule<'a> {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let mut members = Vec::new();
    let mut top_dir = "";
    let mut top_label = "";
    let mut best_prio = (0u8, 0u32);
    for &nid in nodes {
        if let Some(s) = ingest.sym(nid) {
            let n = view.sym_name(s);
            let p = view.sym_path(s);
            let prio = community_anchor_priority(&view, s);
            if top_label.is_empty() || prio > best_prio {
                best_prio = prio;
                top_dir = p;
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

pub fn collect_community_bridges<'a>(
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
    let pr_iters = graph
        .page_rank(&crate::rank::PageRankConfig::default())
        .iterations
        .max(1);
    let mut sum = crate::serialize::CommunitySummary {
        symbols: ingest.symbols.len(),
        connected_singletons,
        pr_iters,
        gauges: graph.gauges(),
        ..Default::default()
    };
    tally_isolated_symbols(ingest, graph, &mut sum);
    sum.bridges = collect_community_bridges(comms, modules, ingest, graph);
    sum
}

pub fn build_ranked_communities<'a>(
    target_id: Option<u32>,
    ingest: &'a IngestResult,
    graph: &Graph,
    interner: &'a Interner,
) -> (
    Vec<u32>,
    std::collections::HashMap<u32, Vec<NodeId>>,
    Vec<crate::serialize::CommunityModule<'a>>,
) {
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
    let modules = comm_ids
        .into_iter()
        .map(|id| build_community_module(id, &by_comm[&id], ingest, interner))
        .collect();
    (comms.comm, by_comm, modules)
}

pub fn execute_communities_query<W: Write>(
    root: &str,
    target_id: Option<u32>,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let (comms, by_comm, modules) = build_ranked_communities(target_id, ingest, graph, interner);
    let summary = compute_community_summary(&comms, &by_comm, &modules, ingest, graph);
    crate::serialize::serialize_communities_with_summary_xml(root, &modules, &summary, out)
}

fn enqueue_around_neighbors(
    curr: NodeId,
    next_d: u32,
    fanout: usize,
    graph: &Graph,
    visited: &mut std::collections::HashSet<NodeId>,
    queue: &mut std::collections::VecDeque<(NodeId, u32)>,
) -> usize {
    let mut cut = 0usize;
    for (idx, (callee_id, _)) in graph.callees(curr).enumerate() {
        if idx < fanout {
            if visited.insert(callee_id) {
                queue.push_back((callee_id, next_d));
            }
        } else {
            cut += 1;
        }
    }
    for (idx, (caller_id, _)) in graph.callers(curr).enumerate() {
        if idx < fanout {
            if visited.insert(caller_id) {
                queue.push_back((caller_id, next_d));
            }
        } else {
            cut += 1;
        }
    }
    cut
}

fn collect_around_callees<'a>(
    curr: NodeId,
    fanout: usize,
    visited: &std::collections::HashSet<NodeId>,
    ingest: &'a IngestResult,
    graph: &Graph,
    interner: &'a Interner,
) -> Vec<&'a str> {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let mut callees = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (callee_id, _) in graph.callees(curr) {
        if visited.contains(&callee_id)
            && let Some(cs) = ingest.sym(callee_id)
        {
            let cn = view.sym_name(cs);
            if seen.insert(cn) {
                callees.push(cn);
                if callees.len() >= fanout {
                    break;
                }
            }
        }
    }
    callees
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
    use crate::domain::CorpusView;
    use crate::navigate::SymbolLookup;
    let view = (ingest, interner);
    let ids = view.find_symbols(sym_name);
    if ids.is_empty() {
        return Err(io::Error::other(format!(
            "ripwire-rs: --around symbol not found: {sym_name}"
        )));
    }
    let sym_id = ids
        .iter()
        .copied()
        .find(|&id| ingest.sym(id).is_some_and(|s| view.is_production_symbol(s)))
        .unwrap_or(ids[0]);
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    visited.insert(sym_id);
    queue.push_back((sym_id, 0u32));

    let mut ordered_nodes = Vec::new();
    let mut fanout_cut = 0usize;
    while let Some((curr, curr_d)) = queue.pop_front() {
        ordered_nodes.push((curr, curr_d));
        if curr_d < cfg.depth {
            fanout_cut += enqueue_around_neighbors(
                curr,
                curr_d + 1,
                cfg.fanout,
                graph,
                &mut visited,
                &mut queue,
            );
        }
    }

    let mut rows = Vec::with_capacity(ordered_nodes.len());
    for (curr, hop) in ordered_nodes {
        if let Some(s) = ingest.sym(curr) {
            rows.push(crate::serialize::AroundSymbolRow {
                name: view.sym_name(s),
                scope: view.sym_scope(s),
                kind: s.kind.tag(),
                path: view.sym_path(s),
                hop,
                callees: collect_around_callees(
                    curr, cfg.fanout, &visited, ingest, graph, interner,
                ),
            });
        }
    }

    crate::serialize::serialize_around_rows_xml(
        root, sym_name, cfg.depth, cfg.fanout, fanout_cut, &rows, out,
    )
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
        let is_call = matches!(
            r.role,
            crate::domain::RefRole::Call | crate::domain::RefRole::Macro
        );
        let name = interner.resolve(&r.callee_name);
        if is_call
            && !by_name.contains_key(&r.callee_name)
            && !r.lang.semantics().is_std_or_builtin_member(name)
            && !r.lang.semantics().is_builtin_keyword(name)
        {
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

fn record_rust_mod_edges(
    ingest: &IngestResult,
    interner: &Interner,
    file_index: &std::collections::HashMap<&str, u32>,
    adj_sets: &mut [std::collections::BTreeSet<usize>],
) {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let f_len = ingest.files.len();
    for s in &ingest.symbols {
        let uf = s.file_id as usize;
        if s.kind == crate::domain::SymKind::Section
            && s.lang == crate::domain::Lang::Rust
            && uf < f_len
        {
            let mod_spec = format!("self::{}", view.sym_name(s));
            if let Some(vf) =
                s.lang
                    .semantics()
                    .resolve_import_target(view.sym_path(s), &mod_spec, file_index)
                && (vf as usize) != uf
                && (vf as usize) < f_len
            {
                adj_sets[uf].insert(vf as usize);
            }
        }
    }
}

fn record_import_edges<'a>(
    ingest: &'a IngestResult,
    interner: &'a Interner,
    file_index: &std::collections::HashMap<&str, u32>,
    adj_sets: &mut [std::collections::BTreeSet<usize>],
    inc_targets: &mut [Vec<&'a str>],
) {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
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
        let f_path = view.file_path(r.file_id);
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
    record_rust_mod_edges(ingest, interner, file_index, adj_sets);
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

    let has_import_edges = adj_sets.iter().any(|s| !s.is_empty());
    if !has_import_edges {
        for u in 0..graph.node_count() as NodeId {
            let uf = ingest.symbols[u as usize].file_id as usize;
            for (v, _, is_split) in graph.callees_with_prov(u) {
                let vf = ingest.symbols[v as usize].file_id as usize;
                if !is_split && uf != vf && uf < f_len && vf < f_len {
                    adj_sets[uf].insert(vf);
                }
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
        limit: None,
        offset: None,
    }
}

pub fn execute_deps_query<W: Write>(
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    execute_deps_query_paged(
        root,
        ingest,
        graph,
        interner,
        &super::args::TopModifierArgs::default(),
        out,
    )
}

pub fn execute_deps_query_paged<W: Write>(
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    page: &dyn super::args::Paginated,
    out: &mut W,
) -> io::Result<()> {
    let mut rep = compute_deps_report(ingest, graph, interner);
    rep.limit = page.limit_opt();
    rep.offset = page.offset_opt();
    crate::serialize::serialize_deps_report_xml(root, &rep, out)
}
