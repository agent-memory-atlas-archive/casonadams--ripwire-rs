use super::args::*;
use super::queries::*;
use super::safety::*;
use super::stream::*;
use crate::domain::Interner;
use crate::graph::Graph;
use crate::mcp::run_mcp_stdio;
use crate::rank::PageRankResult;
use clap::Parser;
use std::io::{self, Write};
use std::path::Path;
pub fn print_symbol_nav_to<W: Write>(
    id: crate::domain::NodeId,
    args: &NavigateArgs,
    sym_name: &str,
    ctx: &NavContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    if args.impact {
        let impact = crate::navigate::transitive_impact(
            id,
            ctx.ingest,
            ctx.graph,
            ctx.ranks,
            ctx.interner,
            None,
        );
        return crate::navigate::serialize_impact_xml(sym_name, &impact, out);
    }
    let Some(nav) =
        crate::navigate::navigate_1hop(id, ctx.ingest, ctx.graph, ctx.ranks, ctx.interner)
    else {
        return Ok(());
    };
    if args.callers {
        crate::navigate::serialize_callers_xml(sym_name, &nav.callers, out)
    } else if args.callees {
        crate::navigate::serialize_callees_xml(sym_name, &nav.callees, out)
    } else {
        crate::navigate::serialize_find_symbol_xml(&nav, out)
    }
}

pub fn execute_navigate_with<W: Write>(
    args: &NavigateArgs,
    ctx: &NavContext<'_>,
    out: &mut W,
) -> io::Result<()> {
    let sym_name = match args.symbol.as_deref() {
        Some(s) => s,
        None => {
            writeln!(out, "Error: --symbol is required for navigate commands")?;
            return Ok(());
        }
    };

    let ids = crate::navigate::find_symbols_by_name(sym_name, ctx.ingest, ctx.interner);
    if ids.is_empty() {
        writeln!(out, "Symbol '{sym_name}' not found")?;
        return Ok(());
    }

    if args.callers || args.callees || args.impact {
        if args.impact {
            let impact = crate::navigate::transitive_impact_multi(
                &ids,
                ctx.ingest,
                ctx.graph,
                ctx.ranks,
                ctx.interner,
                None,
            );
            crate::navigate::serialize_impact_xml(sym_name, &impact, out)?;
        } else if let Some(nav) = crate::navigate::navigate_1hop_multi(
            &ids,
            ctx.ingest,
            ctx.graph,
            ctx.ranks,
            ctx.interner,
        ) {
            if args.callers {
                crate::navigate::serialize_callers_xml(sym_name, &nav.callers, out)?;
            } else {
                crate::navigate::serialize_callees_xml(sym_name, &nav.callees, out)?;
            }
        }
        return Ok(());
    } else if let Some(&first_id) = ids.first() {
        print_symbol_nav_to(first_id, args, sym_name, ctx, out)?;
    }
    Ok(())
}

pub fn execute_navigate(args: &NavigateArgs) -> io::Result<()> {
    let root = args.path.as_deref().unwrap_or(".");
    let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
    let ctx = NavContext {
        ingest: &ingest,
        graph: &graph,
        ranks: &pr.ranks,
        interner: &interner,
    };
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
    execute_navigate_with(args, &ctx, &mut stdout)?;
    stdout.flush()
}

pub fn execute_html_export(
    root: &str,
    out_file: Option<&str>,
    color_by_str: &str,
) -> io::Result<()> {
    let color_by = crate::serialize::ColorBy::from_str_opt(color_by_str)
        .unwrap_or(crate::serialize::ColorBy::Lang);
    let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
    if let Some(path) = out_file {
        let mut file = std::fs::File::create(path)?;
        crate::serialize::html::write_call_graph_html(
            &mut file, &ingest, &graph, &pr.ranks, &interner, color_by, None,
        )?;
    } else {
        let mut stdout = io::stdout().lock();
        crate::serialize::html::write_call_graph_html(
            &mut stdout,
            &ingest,
            &graph,
            &pr.ranks,
            &interner,
            color_by,
            None,
        )?;
    }
    Ok(())
}

pub fn execute_stream_xml(root: &str) -> io::Result<()> {
    let (ingest, graph, pr, interner) = load_repo_analysis(root)?;
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
    execute_stream_xml_with(root, &ingest, &graph, &pr, &interner, &mut stdout)?;
    stdout.flush()
}

pub fn load_repo_analysis(
    root: &str,
) -> io::Result<(crate::domain::IngestResult, Graph, PageRankResult, Interner)> {
    load_repo_analysis_cached(root, None, false)
}

pub fn load_repo_analysis_cached(
    root: &str,
    cache_path: Option<&Path>,
    no_cache: bool,
) -> io::Result<(crate::domain::IngestResult, Graph, PageRankResult, Interner)> {
    if !no_cache && let Some(cached) = crate::cache::load_cached_analysis(root, cache_path) {
        return Ok(cached);
    }
    let path = Path::new(root);
    let files = crate::crawler::crawl_directory(path, &crate::crawler::CrawlerOptions::default());
    let registry =
        crate::ingest::LanguageRegistry::new().map_err(|e| io::Error::other(e.to_string()))?;
    let interner = Interner::new();
    let ingest = crate::ingest::ingest_crawled_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&crate::rank::PageRankConfig::default());
    if !no_cache {
        let _ =
            crate::cache::save_cached_analysis(root, cache_path, (&ingest, &graph, &pr, &interner));
    }
    Ok((ingest, graph, pr, interner))
}

pub fn load_repo_analysis_for_cli(
    cli: &Cli,
    root: &str,
) -> io::Result<(crate::domain::IngestResult, Graph, PageRankResult, Interner)> {
    let cache_path = cli.modifiers.cache.as_deref().map(Path::new);
    let no_cache = cli.modifiers.no_cache;
    load_repo_analysis_cached(root, cache_path, no_cache)
}

pub fn compute_effective_budget(
    pack_budget: Option<usize>,
    token_budget: Option<usize>,
    max_tokens: Option<usize>,
) -> Option<usize> {
    if let Some(tb) = token_budget.or(max_tokens) {
        if tb <= 200 {
            Some(tb.saturating_mul(4))
        } else {
            Some(tb.saturating_mul(2).saturating_sub(1100).max(512))
        }
    } else {
        pack_budget.or(Some(crate::serialize::DEFAULT_PACK_BUDGET_BYTES))
    }
}

pub fn dispatch_top_nav(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let (sym, mode) = if let Some(s) = cli.nav.callers.as_deref() {
        (s, NavMode::Callers)
    } else if let Some(s) = cli.nav.callees.as_deref() {
        (s, NavMode::Callees)
    } else if let Some(s) = cli.nav.impact.as_deref() {
        (s, NavMode::Impact)
    } else if let Some(s) = cli.nav.uses.as_deref() {
        (s, NavMode::Uses)
    } else {
        let s = cli.nav.find_symbol.as_deref()?;
        (s, NavMode::FindSymbol)
    };

    Some((|| {
        let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root)?;
        let mut stdout_lock = io::stdout().lock();
        if cli.modifiers.json {
            let ids = crate::navigate::find_symbols_by_name(sym, &ingest, &interner);
            if let Some(&id) = ids.first() {
                if mode == NavMode::Impact {
                    let impact = crate::navigate::transitive_impact(
                        id, &ingest, &graph, &pr.ranks, &interner, None,
                    );
                    serde_json::to_writer_pretty(&mut stdout_lock, &impact)?;
                    writeln!(stdout_lock)?;
                    return Ok(());
                } else if let Some(nav) =
                    crate::navigate::navigate_1hop(id, &ingest, &graph, &pr.ranks, &interner)
                {
                    serde_json::to_writer_pretty(&mut stdout_lock, &nav)?;
                    writeln!(stdout_lock)?;
                    return Ok(());
                }
            }
        }
        let mut stdout = crate::serialize::XmlWriter::with_options(
            &mut stdout_lock,
            cli.modifiers.is_pretty(),
            cli.modifiers.is_json(),
            cli.legend.eq_ignore_ascii_case("none"),
        );
        let nav_ctx = NavContext {
            ingest: &ingest,
            graph: &graph,
            ranks: &pr.ranks,
            interner: &interner,
        };
        execute_nav_symbol_query_in_root(root, sym, mode, &nav_ctx, &mut stdout)?;
        stdout.flush()
    })())
}

pub fn dispatch_top_retrieval(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    if !cli.retrieval.is_active() {
        return None;
    }
    Some((|| {
        let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root)?;
        let mut stdout_lock = io::stdout().lock();
        let mut stdout = crate::serialize::XmlWriter::with_options(
            &mut stdout_lock,
            cli.modifiers.is_pretty(),
            cli.modifiers.is_json(),
            cli.legend.eq_ignore_ascii_case("none"),
        );
        execute_retrieval_dispatch(
            &cli.retrieval,
            root,
            &ingest,
            &graph,
            &pr,
            &interner,
            &mut stdout,
        )?;
        stdout.flush()
    })())
}

pub fn dispatch_top_workspace(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let ws = &cli.workspace;
    if !ws.is_active() {
        return None;
    }
    Some((|| {
        let mut stdout_lock = io::stdout().lock();
        let mut stdout = crate::serialize::XmlWriter::with_options(
            &mut stdout_lock,
            cli.modifiers.is_pretty(),
            cli.modifiers.is_json(),
            cli.legend.eq_ignore_ascii_case("none"),
        );
        if ws.owners {
            execute_workspace_owners(cli, root, &mut stdout)?;
        } else if ws.stray_content {
            let val = crate::mcp::doc_tools::call_stray_content(root);
            crate::serialize::serialize_stray_content_xml(&val, &mut stdout)?;
        } else {
            let (ingest, _graph, _pr, interner) = load_repo_analysis_for_cli(cli, root)?;
            if let Some((spec, op)) = ws
                .replace_symbol_body
                .as_deref()
                .map(|s| (s, crate::mcp::edit::EditOp::ReplaceBody))
                .or_else(|| {
                    ws.insert_before_symbol
                        .as_deref()
                        .map(|s| (s, crate::mcp::edit::EditOp::InsertBefore))
                })
                .or_else(|| {
                    ws.insert_after_symbol
                        .as_deref()
                        .map(|s| (s, crate::mcp::edit::EditOp::InsertAfter))
                })
            {
                execute_edit_op_with_options(
                    EditCliOptions {
                        root,
                        spec,
                        op,
                        payload_override: cli.safety.edit_payload.as_deref(),
                        dry_run: cli.safety.dry_run,
                    },
                    &ingest,
                    &interner,
                    &mut stdout,
                )?;
            } else {
                execute_workspace_doc_query(
                    ws,
                    root,
                    &ingest,
                    &interner,
                    cli.retrieval.with_history,
                    &mut stdout,
                )?;
            }
        }
        stdout.flush()
    })())
}

fn execute_workspace_owners<W: Write>(cli: &Cli, root: &str, out: &mut W) -> io::Result<()> {
    let cutoff = crate::git::history::resolve_cutoff_timestamp(None, 12);
    let commits = crate::git::history::walk_repository_history_scoped(Path::new(root), cutoff).ok();
    let at_stamp = crate::git::history::compute_git_at_stamp(root);
    let (ingest, _graph, _pr, interner) = load_repo_analysis_for_cli(cli, root)?;
    let known_files: std::collections::HashSet<String> = ingest
        .files
        .iter()
        .map(|f| interner.resolve(&f.path).to_string())
        .collect();
    let mut val = crate::mcp::doc_tools::call_owners(
        commits.as_deref(),
        None,
        at_stamp.as_deref(),
        Some(&known_files),
    );
    if cli.retrieval.detail.is_some_and(|d| d > 0) {
        val["detail"] = serde_json::Value::Bool(true);
    }
    crate::serialize::serialize_owners_xml(&val, out)
}

fn cap_lint_findings_per_rule(
    findings: &mut Vec<crate::metrics::lint::LintFinding>,
    max_per_rule: Option<usize>,
) {
    let Some(limit) = max_per_rule else {
        return;
    };
    let mut rule_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    findings.retain(|f| {
        let c = rule_counts.entry(f.rule.clone()).or_insert(0);
        if *c < limit {
            *c += 1;
            true
        } else {
            false
        }
    });
}

fn execute_detail_extended<W: Write>(
    cli: &Cli,
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    stdout: &mut W,
) -> io::Result<()> {
    let detail = &cli.detail;
    if detail.lint {
        let mut findings = crate::metrics::lint::run_lint_scan(ingest, graph, interner);
        cap_lint_findings_per_rule(&mut findings, detail.lint_max_per_rule);
        if cli.arch.sarif {
            crate::metrics::lint::serialize_lint_sarif(&findings, stdout)
        } else {
            crate::metrics::lint::serialize_lint_xml(root, &findings, stdout)
        }
    } else if detail.lint_catalog {
        crate::metrics::lint::serialize_lint_catalog(stdout)
    } else if detail.naming_locals {
        crate::metrics::serialize_naming_consistency_xml(root, true, ingest, interner, stdout)
    } else if detail.naming_consistency {
        crate::metrics::serialize_naming_consistency_xml(root, false, ingest, interner, stdout)
    } else if detail.naming_calibration {
        crate::metrics::serialize_naming_calibration_xml(root, ingest, interner, stdout)
    } else if detail.report {
        crate::metrics::serialize_executive_report_md(
            root, ingest, graph, &pr.ranks, interner, stdout,
        )
    } else if detail.legend_dict {
        crate::serialize::serialize_legend_dict_xml(stdout)
    } else {
        Ok(())
    }
}

pub fn dispatch_top_detail(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let detail = &cli.detail;
    if !detail.is_active() {
        return None;
    }

    Some((|| {
        let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root)?;
        let mut stdout_lock = io::stdout().lock();
        let mut stdout = crate::serialize::XmlWriter::with_options(
            &mut stdout_lock,
            cli.modifiers.is_pretty() || detail.report,
            cli.modifiers.is_json() && !detail.report,
            cli.legend.eq_ignore_ascii_case("none") && !detail.report,
        );

        if let Some(syms) = detail.expand.as_deref() {
            execute_expand_or_outline_query(root, syms, false, &ingest, &interner, &mut stdout)?;
        } else if let Some(syms) = detail.outline.as_deref() {
            execute_expand_or_outline_query(root, syms, true, &ingest, &interner, &mut stdout)?;
        } else if let Some(n) = detail.pack_top_n {
            execute_pack_top_n_query(root, n, &ingest, &pr, &interner, &mut stdout)?;
        } else if detail.tree {
            execute_tree_query(root, &ingest, &pr, &interner, &mut stdout)?;
        } else if detail.biggest_first {
            crate::metrics::readability::execute_biggest_first_query(
                root,
                &ingest,
                &interner,
                &mut stdout,
            )?;
        } else if detail.nonlocal_state {
            let funcs = crate::metrics::nonlocal::find_nonlocal_state(&ingest, &graph, &interner);
            crate::metrics::nonlocal::serialize_nonlocal_state_xml(root, &funcs, &mut stdout)?;
        } else {
            execute_detail_extended(cli, root, &ingest, &graph, &pr, &interner, &mut stdout)?;
        }
        stdout.flush()?;
        Ok(())
    })())
}

pub fn dispatch_top_safety(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let s = &cli.safety;
    if !s.is_active() {
        return None;
    }
    Some((|| {
        let (ingest, graph, _pr, interner) = load_repo_analysis_for_cli(cli, root)?;
        let mut stdout_lock = io::stdout().lock();
        let mut stdout = crate::serialize::XmlWriter::with_options(
            &mut stdout_lock,
            cli.modifiers.is_pretty(),
            cli.modifiers.is_json(),
            cli.legend.eq_ignore_ascii_case("none"),
        );
        let res = execute_safety_dispatch(s, root, &ingest, &graph, &interner, &mut stdout);
        stdout.flush()?;
        if let Some(code) = res? {
            std::process::exit(code);
        }
        Ok(())
    })())
}
fn execute_arch_rules_query<W: Write>(
    rules_file: &str,
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    interner: &Interner,
    stdout: &mut W,
) -> io::Result<()> {
    let content = std::fs::read_to_string(rules_file)?;
    let cfg = crate::gate::arch::ArchConfig::parse(&content);
    let violations = crate::gate::arch::check_architecture_rules(&cfg, ingest, graph, interner);
    crate::gate::arch::serialize_arch_xml(root, &violations, stdout)?;
    if !violations.is_empty() {
        std::process::exit(2);
    }
    Ok(())
}

pub fn dispatch_top_arch(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    let a = &cli.arch;
    if !a.is_active() {
        return None;
    }

    Some((|| {
        let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root)?;
        let mut stdout_lock = io::stdout().lock();

        if a.mermaid {
            return crate::graph::generate_mermaid_diagram(
                &ingest,
                &graph,
                &interner,
                &mut stdout_lock,
            );
        }

        let mut stdout = crate::serialize::XmlWriter::with_options(
            &mut stdout_lock,
            cli.modifiers.is_pretty(),
            cli.modifiers.is_json(),
            cli.legend.eq_ignore_ascii_case("none"),
        );

        if let Some(rules_file) = a.arch.as_deref() {
            execute_arch_rules_query(rules_file, root, &ingest, &graph, &interner, &mut stdout)?;
        } else if a.zoom.is_some() {
            execute_zoom_query(&ingest, &graph, &interner, &mut stdout)?;
        } else if a.dead_code {
            execute_dead_code_query(root, &ingest, &graph, &interner, &mut stdout)?;
        } else if a.communities || a.community.is_some() {
            execute_communities_query(root, a.community, &ingest, &graph, &interner, &mut stdout)?;
        } else if let Some(sym) = a.around.as_deref() {
            let cfg = AroundConfig {
                depth: a.around_depth,
                fanout: a.around_fanout,
            };
            execute_around_query(root, sym, cfg, &ingest, &graph, &interner, &mut stdout)?;
        } else if a.external_surface {
            execute_external_surface_query(root, &ingest, &interner, &mut stdout)?;
        } else if a.deps {
            execute_deps_query(root, &ingest, &graph, &interner, &mut stdout)?;
        } else if dispatch_lens_queries(cli, root, &ingest, &graph, &pr, &interner, &mut stdout)? {
            stdout.flush()?;
            return Ok(());
        }
        stdout.flush()?;
        Ok(())
    })())
}

fn dispatch_lens_queries<W: Write>(
    cli: &Cli,
    root: &str,
    ingest: &crate::domain::IngestResult,
    graph: &Graph,
    pr: &crate::rank::PageRankResult,
    interner: &Interner,
    stdout: &mut W,
) -> io::Result<bool> {
    let a = &cli.arch;
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
        crate::metrics::serialize_quality_panel_xml(&rep, stdout)?;
        return Ok(true);
    }
    if let Some(sym) = a.layout.as_deref() {
        let has_c_agg = ingest
            .symbols
            .iter()
            .any(|s| interner.resolve(&s.name) == sym && s.lang.semantics().supports_c_layout());
        if !has_c_agg {
            return Err(io::Error::other(format!(
                "ripwire: --layout: '{sym}' is indexed but has no C-family aggregate body — this verb models C/C++/ObjC byte layout only"
            )));
        }
        let rep = crate::metrics::compute_layout_report(sym, ingest, interner, root);
        crate::metrics::serialize_layout_xml(&rep, root, stdout)?;
        return Ok(true);
    }
    if let Some(_sym_opt) = &a.field_affinity {
        let rep = crate::metrics::StructLayoutReport {
            sym: String::new(),
            found: false,
            defs: Vec::new(),
        };
        crate::metrics::serialize_field_affinity_xml(&rep, root, stdout)?;
        return Ok(true);
    }
    if a.context_ratio {
        let rep = crate::metrics::compute_context_ratio(ingest, graph, interner);
        crate::metrics::serialize_context_ratio_xml(&rep, root, stdout)?;
        return Ok(true);
    }
    if a.ensemble {
        let rep = crate::metrics::compute_ensemble(ingest, graph, interner);
        crate::metrics::serialize_ensemble_xml(&rep, root, stdout)?;
        return Ok(true);
    }
    if a.comment_coherence {
        let rep = crate::metrics::compute_comment_coherence(ingest, interner);
        crate::metrics::serialize_comment_coherence_xml(&rep, root, stdout)?;
        return Ok(true);
    }
    Ok(false)
}

pub fn dispatch_top_flags(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    if cli.modifiers.eval {
        let mut stdout = io::stdout().lock();
        return Some(crate::metrics::run_cochange_eval(&mut stdout));
    }
    if cli.modifiers.eval_retrieval {
        let mut stdout = io::stdout().lock();
        return Some(crate::metrics::run_retrieval_eval(&mut stdout));
    }
    if let Some(file) = cli.modifiers.batch.as_deref() {
        return Some((|| {
            let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root)?;
            let mut stdout_lock = io::stdout().lock();
            let mut stdout = crate::serialize::XmlWriter::with_options(
                &mut stdout_lock,
                cli.modifiers.is_pretty(),
                cli.modifiers.is_json(),
                cli.legend.eq_ignore_ascii_case("none"),
            );
            execute_batch_file(root, file, &ingest, &graph, &pr, &interner, &mut stdout)?;
            stdout.flush()
        })());
    }
    dispatch_top_nav(cli, root)
        .or_else(|| dispatch_top_retrieval(cli, root))
        .or_else(|| dispatch_top_workspace(cli, root))
        .or_else(|| dispatch_top_detail(cli, root))
        .or_else(|| dispatch_top_safety(cli, root))
        .or_else(|| dispatch_top_arch(cli, root))
}

pub fn dispatch_default_run(cli: &Cli, root: &str, budget: Option<usize>) -> io::Result<()> {
    let emit_legend = !cli.legend.eq_ignore_ascii_case("none");
    let legend_full = cli.legend.eq_ignore_ascii_case("full");
    let (ingest, graph, pr, interner) = load_repo_analysis_for_cli(cli, root)?;
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::with_options(
        &mut stdout_lock,
        cli.modifiers.is_pretty(),
        cli.modifiers.is_json(),
        cli.legend.eq_ignore_ascii_case("none"),
    );
    let args = RankedStreamArgs {
        root,
        rank_by: &cli.rank_by,
        pack_budget: budget,
        top_k: cli.top_k.or(Some(200)),
        emit_legend,
        legend_full,
        map_diff: cli.safety.map_diff,
    };
    execute_stream_xml_ranked(&args, &ingest, &graph, &pr, &interner, &mut stdout)?;
    stdout.flush()
}
fn execute_orient_subcommand(orient_args: &OrientArgs) -> io::Result<()> {
    let root = orient_args.path.as_deref().unwrap_or(".");
    let Some(task) = orient_args.for_task.as_deref() else {
        return execute_stream_xml(root);
    };
    let (ingest, graph, pr, interner) = load_repo_analysis_cached(root, None, false)?;
    let val = crate::mcp::retrieval::call_for_task_with_options(
        &ingest,
        &interner,
        &pr.ranks,
        task,
        crate::mcp::retrieval::ForTaskOptions {
            root,
            graph: Some(&graph),
            ..Default::default()
        },
    );
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
    crate::serialize::serialize_for_xml(&val, &mut stdout)?;
    stdout.flush()
}

pub fn dispatch_subcommand(command: &Commands) -> io::Result<()> {
    match command {
        Commands::Mcp(mcp_args) => run_mcp_stdio(mcp_args.path.as_deref().unwrap_or(".")),
        Commands::Navigate(nav_args) => execute_navigate(nav_args),
        Commands::Orient(orient_args) => execute_orient_subcommand(orient_args),
        Commands::Gate(gate_args) => {
            let root = gate_args.path.as_deref().unwrap_or(".");
            let sub_gate = GateDispatchArgs {
                edit_check: gate_args.edit_check.as_deref(),
                affected: gate_args.affected.as_deref(),
                quality_baseline: gate_args.quality_baseline,
                quality_delta: gate_args.quality_delta,
            };
            if let Some(res) = dispatch_gate_command(root, sub_gate) {
                return res;
            }
            if gate_args.clones {
                return crate::clones::execute_clones_command(root, gate_args.min_tokens);
            }
            Ok(())
        }
        Commands::Metrics(metrics_args) => {
            let root = metrics_args.path.as_deref().unwrap_or(".");
            if let Some(res) = dispatch_git_metrics_command(
                root,
                metrics_args.cochange.as_ref().map(|o| o.as_deref()),
                metrics_args.hotspots,
                metrics_args.since.as_deref(),
            ) {
                return res;
            }
            Ok(())
        }
        Commands::Slice(slice_args) => {
            let target = slice_args.target.as_deref();
            crate::slice::execute_slice_command(".", target, &slice_args.flow)
        }
        Commands::Skills(skills_args) => {
            let target = skills_args.path.as_deref();
            execute_skills_install(target)
        }
        Commands::Wrap(wrap_args) => {
            crate::cli::wrap::execute_wrap_command(&wrap_args.agent, wrap_args.force)
        }
    }
}
pub fn execute_skills_install(target: Option<&str>) -> io::Result<()> {
    let report = crate::skills::install_skills(target)?;
    println!(
        "Installed {} ripwire-rs agent skills to {}",
        report.installed_count,
        report.destination.display()
    );
    println!(
        "Active manifest written to {}",
        report.destination.join(".ripwire-rs-manifest-v1").display()
    );
    for skill in &report.skills {
        println!("  - {skill}");
    }
    Ok(())
}

/// Execute CLI from parsed options.
fn dispatch_primary_workflows(cli: &Cli, root: &str) -> Option<io::Result<()>> {
    if cli.mcp {
        return Some(run_mcp_stdio(root));
    }
    if cli.modifiers.lsp {
        return Some(crate::mcp::run_lsp_stdio(root));
    }
    if let Some(target_opt) = &cli.add_skills {
        let target = target_opt.as_deref();
        return Some(execute_skills_install(target));
    }
    if let Some(res) = dispatch_top_flags(cli, root) {
        return Some(res);
    }
    if let Some(res) = dispatch_git_metrics_with_flags(
        root,
        GitMetricsFlags {
            cochange: cli.cochange.as_ref().map(|o| o.as_deref()),
            cochange_groups: cli.safety.cochange_groups,
            hotspots: cli.hotspots,
            since: cli.since.as_deref(),
            pretty: cli.modifiers.is_pretty(),
            json_out: cli.modifiers.is_json(),
        },
    ) {
        return Some(res);
    }
    None
}

pub fn run_cli_args(cli: Cli) -> io::Result<()> {
    let root = cli.path.as_deref().unwrap_or(".");
    if let Some(res) = dispatch_primary_workflows(&cli, root) {
        return res;
    }

    let cli_gate = GateDispatchArgs {
        edit_check: cli.edit_check.as_deref(),
        affected: cli.affected.as_deref(),
        quality_baseline: cli.quality_baseline,
        quality_delta: cli.quality_delta,
    };
    if let Some(res) = dispatch_gate_command(root, cli_gate) {
        return res;
    }

    if cli.clones {
        return crate::clones::execute_clones_command(root, cli.min_tokens);
    }
    if let Some(Commands::Gate(gate_args)) = &cli.command
        && gate_args.clones
    {
        let gate_root = gate_args.path.as_deref().unwrap_or(root);
        return crate::clones::execute_clones_command(gate_root, gate_args.min_tokens);
    }

    if let Some(html_opt) = cli.html {
        return execute_html_export(root, html_opt.as_deref(), &cli.color_by);
    }

    if let Some(Commands::Slice(slice_args)) = &cli.command {
        return crate::slice::execute_slice_command(
            root,
            slice_args.target.as_deref(),
            &slice_args.flow,
        );
    }

    if let Some(target) = cli.slice.as_deref() {
        return crate::slice::execute_slice_command(root, Some(target), &cli.slice_flow);
    }

    let budget = if cli.top_k.is_some()
        && cli.pack_budget.is_none()
        && cli.token_budget.is_none()
        && cli.max_tokens.is_none()
    {
        None
    } else {
        compute_effective_budget(cli.pack_budget, cli.token_budget, cli.max_tokens)
    };
    match &cli.command {
        Some(cmd) => dispatch_subcommand(cmd),
        None => dispatch_default_run(&cli, root, budget),
    }
}

/// Main CLI entry point.
pub fn run_cli() -> io::Result<()> {
    let cli = Cli::parse();
    run_cli_args(cli)
}
