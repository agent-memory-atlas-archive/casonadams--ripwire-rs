use super::ArchitectureArgs;
use crate::commands::context::ExecutionContext;
use crate::domain::{IngestResult, Interner, NodeId};
use crate::metrics::architecture::{build_ranked_communities, compute_community_summary, compute_deps_report};
use crate::graph::Graph;
use std::io::{self, Write};

/// Configuration for `--around` ego-graph queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AroundConfig {
    pub depth: u32,
    pub fanout: usize,
}

struct CachedFile<'a> {
    source: std::borrow::Cow<'a, [u8]>,
    tree: Option<tree_sitter::Tree>,
}

fn parse_file_tree(
    lang: crate::domain::Lang,
    path: &str,
    source: &[u8],
) -> Option<tree_sitter::Tree> {
    let grammar = lang.semantics().source_grammar(path)?;
    crate::ingest::registry::PARSER_STATE.with(|state| {
        let mut borrowed = state.borrow_mut();
        let (parser, _) = &mut *borrowed;
        parser.set_language(grammar).ok()?;
        parser.parse(source, None)
    })
}

pub struct DeadCodeContext<'a> {
    pub root: &'a str,
    pub sources: &'a dyn crate::query::SourceReader,
    pub ingest: &'a IngestResult,
    pub graph: &'a Graph,
    pub interner: &'a Interner,
    pub page: &'a dyn crate::commands::context::Paginated,
}

pub fn execute_dead_code_query_paged<W: Write>(
    ctx: &DeadCodeContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    use crate::domain::CorpusView;
    let view = (ctx.ingest, ctx.interner);
    let mut file_cache: std::collections::HashMap<u32, CachedFile<'_>> =
        std::collections::HashMap::new();
    let mut dead = Vec::new();
    for sym in &ctx.ingest.symbols {
        if sym.kind != crate::domain::SymKind::Function
            || !sym.lang.semantics().supports_internal_linkage()
            || sym.sig_end_byte >= sym.end_byte
            || ctx.graph.in_degree(sym.id) != 0
        {
            continue;
        }
        let p = view.sym_path(sym);
        if sym.lang.semantics().is_header_file(p) {
            continue;
        }
        let cached = file_cache.entry(sym.file_id).or_insert_with(|| {
            let source = ctx
                .sources
                .read(p)
                .unwrap_or(std::borrow::Cow::Borrowed(&[]));
            let tree = parse_file_tree(sym.lang, p, source.as_ref());
            CachedFile { source, tree }
        });
        let is_internal = cached.tree.as_ref().is_some_and(|tree| {
            sym.lang.semantics().is_internal_function(
                tree.root_node(),
                sym.sig_start_byte as usize,
                cached.source.as_ref(),
            )
        });
        if is_internal {
            dead.push((view.sym_name(sym), sym.kind.tag(), p, sym.line));
        }
    }
    dead.sort_by(|a, b| {
        a.2.cmp(b.2)
            .then_with(|| a.3.cmp(&b.3))
            .then_with(|| a.0.cmp(b.0))
    });
    crate::serialize::query_xml::serialize_dead_code_paged_xml(
        ctx.root,
        &dead,
        ctx.graph.gauges(),
        ctx.page.limit_opt(),
        ctx.page.offset_opt(),
        out,
    )
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


pub fn execute_deps_query_paged<W: Write>(
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    page: &dyn crate::commands::context::Paginated,
    out: &mut W,
) -> io::Result<()> {
    let mut rep = compute_deps_report(ingest, graph, interner);
    rep.limit = page.limit_opt();
    rep.offset = page.offset_opt();
    crate::serialize::serialize_deps_report_xml(root, &rep, out)
}

fn execute_arch_rules_query<W: Write>(
    rules_file: &str,
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    stdout: &mut crate::serialize::XmlWriter<'_, W>,
) -> io::Result<()> {
    let content = std::fs::read_to_string(rules_file)?;
    let cfg = crate::gate::arch::ArchConfig::parse(&content);
    let violations = crate::gate::arch::check_architecture_rules(&cfg, ingest, graph, interner);
    crate::gate::arch::serialize_arch_xml(root, &violations, stdout)?;
    stdout.finish()?;
    if !violations.is_empty() {
        std::process::exit(2);
    }
    Ok(())
}

pub fn execute_arch_or_zoom_query<W: Write>(
    arch_args: &ArchitectureArgs,
    ctx: &ExecutionContext<'_>,
    out: &mut crate::serialize::XmlWriter<'_, W>,
) -> Option<io::Result<()>> {
    if arch_args.mermaid {
        return Some(crate::graph::generate_mermaid_diagram(
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            out,
        ));
    }
    if let Some(rules_file) = arch_args.arch.as_deref() {
        return Some(execute_arch_rules_query(
            rules_file,
            ctx.root,
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            out,
        ));
    }
    if let Some(depth) = arch_args.zoom {
        let opts = crate::graph::zoom::ZoomOptions {
            depth,
            levels_shown: arch_args.zoom_levels,
            limit: ctx.cli.modifiers.limit,
            offset: ctx.cli.modifiers.offset,
        };
        return Some(crate::graph::zoom::execute_zoom_query(
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            ctx.pr.iterations,
            opts,
            out,
        ));
    }
    None
}

pub fn dispatch_lens_queries<W: Write>(ctx: &ExecutionContext<'_>, stdout: &mut W) -> io::Result<bool> {
    let (cli, root, ingest, graph, pr, interner) = (
        ctx.cli,
        ctx.root,
        ctx.ingest,
        ctx.graph,
        ctx.pr,
        ctx.interner,
    );
    let a = super::args(cli);
    if a.metrics {
        let opts = crate::serialize::SerializeOptions {
            root: root.to_string(),
            top_k: cli.top_k,
            pack_budget_bytes: if cli.top_k.is_some() {
                None
            } else {
                Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES)
            },
            metrics: true,
            ..Default::default()
        };
        crate::serialize::serialize_xml(ingest, graph, pr, interner, stdout, &opts)?;
        return Ok(true);
    }
    if let Some(preset_opt) = &a.quality_panel {
        let rep = crate::metrics::compute_quality_panel_report(
            ingest,
            graph,
            interner,
            preset_opt.as_deref(),
        );
        return ctx.write_report(&rep, stdout);
    }
    if let Some(sym) = a.layout.as_deref() {
        use crate::domain::CorpusView;
        let has_c_agg = ingest
            .symbols
            .iter()
            .any(|s| ctx.sym_name(s) == sym && s.lang.semantics().supports_c_layout());
        if !has_c_agg {
            return Err(io::Error::other(format!(
                "ripwire-rs: --layout: '{sym}' is indexed but has no C-family aggregate body — this verb models C/C++/ObjC byte layout only"
            )));
        }
        let rep = crate::metrics::compute_layout_report(sym, ingest, interner, root);
        return ctx.write_report(&rep, stdout);
    }
    if let Some(sym_opt) = &a.field_affinity {
        let rep = crate::metrics::layout::compute_field_affinity_report(
            sym_opt.as_deref(),
            ingest,
            interner,
            root,
        );
        return ctx.write_report(&rep, stdout);
    }
    if a.context_ratio {
        let rep = crate::metrics::compute_context_ratio(ingest, graph, interner);
        return ctx.write_report(&rep, stdout);
    }
    if a.ensemble {
        let rep = crate::metrics::compute_ensemble(ingest, graph, interner);
        return ctx.write_report(&rep, stdout);
    }
    if a.comment_coherence {
        let rep = crate::metrics::analysis_lenses::compute_comment_coherence_in_root(
            root, ingest, interner,
        );
        return ctx.write_report(&rep, stdout);
    }
    Ok(false)
}

