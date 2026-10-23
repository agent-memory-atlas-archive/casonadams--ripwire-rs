use crate::commands::common::changes::{
    get_working_tree_changed_files, resolve_changed_or_spec_files,
};
use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::query::safety::SafetyContext;
use crate::serialize::ToXmlString;
use std::io::{self, Write};
use std::path::Path;

#[derive(Clone, Copy)]
pub struct TestGateRequest<'a> {
    pub files: &'a [String],
    pub at_stamp: &'a str,
    pub page: crate::serialize::PageRequest,
    pub sources: &'a dyn crate::query::SourceReader,
    pub gates: &'a crate::gate::script_gates::ScriptGates,
}

pub fn execute_test_gate_from_facts<W: Write>(
    ctx: &SafetyContext<'_>,
    req: &TestGateRequest<'_>,
    out: &mut W,
) -> io::Result<bool> {
    use crate::domain::CorpusView;
    use crate::serialize::XmlReport;
    let specs: Vec<&str> = req.files.iter().map(String::as_str).collect();
    let mut report =
        crate::gate::affected::find_affected_tests(&specs, ctx.ingest, ctx.graph, ctx.interner);
    let gate_rows = req.gates.covering_rows(req.files, &report.tests);
    report.tests.extend(gate_rows);
    crate::gate::runners::attach_test_runners_from_sources(
        &mut report.tests,
        req.sources,
        ctx.ingest,
        ctx.interner,
    );
    let tested = crate::navigate::compute_tested_node_mask(ctx.ingest, ctx.graph, ctx.interner);
    let sum = crate::gate::summarize_test_gate(&report, &tested, ctx.ingest, ctx.interner);
    let view = (ctx.ingest, ctx.interner);
    let untested: Vec<crate::serialize::arch_xml::UntestedSymbol<'_>> = sum
        .untested
        .iter()
        .map(|&id| {
            let sym = &ctx.ingest[id];
            crate::serialize::arch_xml::UntestedSymbol {
                name: view.sym_name(sym),
                path: view.sym_path(sym),
                line: sym.line,
                ccx: sym.ccx,
            }
        })
        .collect();
    crate::serialize::arch_xml::TestGateReport {
        root: ctx.root,
        at_stamp: req.at_stamp,
        changed: req.files.len(),
        impacted: sum.impacted,
        tests: &report.tests,
        untested: &untested,
        untested_modscope: sum.untested_modscope,
        run_first: sum.run_first,
        script_gates_unmodelled: req.gates.unmodelled,
        script_gates_registered: req.gates.registered.len(),
        script_gates_mapped: req.gates.mapped(),
        page: req.page,
        gauge_attrs: &ctx.graph.xml_gauge_attrs(),
    }
    .render_envelope(out)?;
    Ok(!report.tests.is_empty() || !sum.untested.is_empty())
}

pub fn execute_test_gate_query_paged<W: Write>(
    root: &str,
    files_spec: Option<&str>,
    page: crate::serialize::PageRequest,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    use crate::serialize::GitStampExt;
    let files = resolve_changed_or_spec_files(root, files_spec);
    let at_stamp = root.git_at_or_head();
    let ctx = SafetyContext {
        root,
        ingest,
        graph,
        interner,
    };
    let sources = crate::query::FileSources {
        root: Path::new(root),
    };
    let gates = crate::gate::script_gates::collect_script_gates(root, ingest, interner);
    let req = TestGateRequest {
        files: &files,
        at_stamp: &at_stamp,
        page,
        sources: &sources,
        gates: &gates,
    };
    execute_test_gate_from_facts(&ctx, &req, out)
}

pub fn execute_situ_from_facts<W: Write>(
    ctx: &SafetyContext<'_>,
    files: &[String],
    commits: Option<&[crate::git::history::CommitInfo]>,
    page: crate::serialize::PageRequest,
    out: &mut W,
) -> io::Result<()> {
    crate::query::safety::SituationalReport {
        root: ctx.root,
        files,
        commits,
        context: *ctx,
        page,
    }
    .write_xml(out)
}

pub fn execute_situ_query<W: Write>(
    ctx: &SafetyContext<'_>,
    files_spec: Option<&str>,
    page: crate::serialize::PageRequest,
    out: &mut W,
) -> io::Result<()> {
    let files = resolve_changed_or_spec_files(ctx.root, files_spec);
    let commits = crate::git::history::walk_repository_history(ctx.root).ok();
    execute_situ_from_facts(ctx, &files, commits.as_deref(), page, out)
}

pub fn execute_safe_delete_query<W: Write>(
    facts: &SafetyContext<'_>,
    sym_name: &str,
    page: crate::serialize::PageRequest,
    out: &mut W,
) -> io::Result<()> {
    let SafetyContext {
        root,
        ingest,
        graph,
        interner,
    } = *facts;
    use crate::domain::CorpusView;
    use crate::navigate::SymbolLookup;
    let view = (ingest, interner);
    let sym = view.find_first_symbol(sym_name).ok_or_else(|| {
        io::Error::other(format!(
            "ripwire-rs: --safe-delete symbol not found: {sym_name}"
        ))
    })?;
    let sym_id = sym.id;
    let def_loc = format!("{}:{}", view.sym_path(sym), sym.line);
    let callers: Vec<(&str, &str)> = graph
        .callers(sym_id)
        .filter_map(|(cid, _)| {
            let s = ingest.sym(cid)?;
            Some((view.sym_path(s), view.sym_name(s)))
        })
        .collect();
    let ranks = vec![0.0; ingest.symbols.len()];
    let blast_radius =
        crate::navigate::transitive_impact(sym_id, ingest, graph, &ranks, interner, Some(32)).len();
    let gauge_attrs = graph.xml_gauge_attrs();
    use crate::serialize::XmlReport;
    crate::serialize::query_xml::SafeDeleteReport {
        root,
        sym_name,
        kind_tag: sym.kind.tag(),
        def_loc: &def_loc,
        callers: &callers,
        blast_radius,
        gauge_attrs: &gauge_attrs,
        page,
    }
    .render_envelope(out)
}

pub fn execute_exercises_query<W: Write>(
    root: &str,
    test_path: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let mut reached = Vec::new();
    for sym in &ingest.symbols {
        let p = ingest.sym_file_path(sym, interner);
        if !p.contains(test_path) {
            continue;
        }
        for (callee_id, _) in graph.callees(sym.id) {
            let Some(target) = ingest.sym(callee_id) else {
                continue;
            };
            let tp = ingest.sym_file_path(target, interner);
            if !crate::gate::affected::is_test_path(tp) {
                reached.push((tp, &interner[target.name]));
            }
        }
    }
    crate::serialize::serialize_exercises_xml(root, test_path, &reached, out)
}

pub fn execute_merge_scout_dispatch<W: Write>(
    root: &str,
    branches: &str,
    out: &mut W,
) -> io::Result<Option<i32>> {
    let refs: Vec<&str> = branches
        .split(',')
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .collect();
    let inputs = match crate::git::merge_scout::collect_scout(root, &refs) {
        Ok(inputs) => inputs,
        Err(msg) => {
            eprintln!("ripwire-rs: --merge-scout: {msg}");
            return Ok(Some(1));
        }
    };
    let rep =
        crate::gate::merge_scout::evaluate_merge_scout(&inputs.head, &inputs.at, &inputs.arms);
    crate::gate::merge_scout::serialize_merge_scout_xml(&rep, out)?;
    Ok(None)
}

pub fn execute_pr_context_from_facts<W: Write>(
    ctx: &SafetyContext<'_>,
    req: &crate::gate::pr_context::PrContextRequest<'_>,
    out: &mut W,
) -> io::Result<crate::gate::pr_context::PrContextReport> {
    let rep = crate::gate::pr_context::compute_pr_context_from_facts(
        req,
        ctx.ingest,
        ctx.graph,
        ctx.interner,
    );
    crate::gate::pr_context::serialize_pr_context_xml(&rep, out)?;
    Ok(rep)
}

pub fn execute_pr_context_dispatch<W: Write>(
    facts: &SafetyContext<'_>,
    ref_opt: Option<&str>,
    page: crate::serialize::PageRequest,
    out: &mut W,
) -> io::Result<()> {
    let SafetyContext { root, .. } = *facts;
    let base_ref = ref_opt.filter(|s| !s.is_empty()).unwrap_or("working-tree");
    let changed = get_working_tree_changed_files(root);
    let commits = crate::git::history::walk_repository_history(root).unwrap_or_default();
    let req = crate::gate::pr_context::PrContextRequest {
        changed_files: &changed,
        base_ref,
        commits: &commits,
        page,
    };
    execute_pr_context_from_facts(facts, &req, out)?;
    Ok(())
}

pub fn resolve_scan_skills_dir(raw: &str) -> std::path::PathBuf {
    if !raw.is_empty() {
        crate::skills::resolve_skills_dir(Some(raw))
    } else if let Ok(codex_home) = std::env::var("CODEX_HOME")
        && std::path::Path::new(&codex_home).join("skills").is_dir()
    {
        std::path::Path::new(&codex_home).join("skills")
    } else if std::path::Path::new("skills").is_dir() {
        std::path::PathBuf::from("skills")
    } else {
        crate::skills::default_skills_dir()
    }
}

pub fn render_skill_scan_report<W: Write>(
    rep: &crate::skills::SkillScanReport,
    out: &mut W,
) -> io::Result<i32> {
    crate::skills::serialize_skill_scan_xml(rep, out)?;
    Ok(match rep.verdict {
        "critical" => 2,
        "warn" => 1,
        _ => 0,
    })
}

pub fn execute_scan_skills_dispatch<W: Write>(raw: &str, out: &mut W) -> io::Result<i32> {
    let scan_dir = resolve_scan_skills_dir(raw);
    let rep = crate::skills::scan_skills_directory(&scan_dir)?;
    eprintln!("{} skill file(s) scanned", rep.files_scanned);
    render_skill_scan_report(&rep, out)
}

pub fn execute_edit_plan_from_facts<W: Write>(
    ctx: &SafetyContext<'_>,
    plan_raw: &str,
    dry_run: bool,
    out: &mut W,
) -> io::Result<i32> {
    let res = crate::query::edit::execute_edit_plan_json(
        ctx.root,
        plan_raw,
        dry_run,
        ctx.ingest,
        ctx.interner,
    );
    writeln!(out, "{res}")?;
    Ok(if res.get("error").is_some() { 1 } else { 0 })
}

pub fn execute_edit_plan_dispatch<W: Write>(
    plan_spec: &str,
    ctx: &SafetyContext<'_>,
    dry_run: bool,
    out: &mut W,
) -> io::Result<i32> {
    let plan_raw = if plan_spec == "-" {
        let mut buf = String::new();
        io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
        buf
    } else {
        std::fs::read_to_string(plan_spec).unwrap_or_else(|_| plan_spec.to_string())
    };
    execute_edit_plan_from_facts(ctx, &plan_raw, dry_run, out)
}
pub fn execute_partition_from_facts<W: Write>(
    ctx: &SafetyContext<'_>,
    partition_k: usize,
    task: Option<&str>,
    page: crate::serialize::PageRequest,
    out: &mut W,
) -> io::Result<()> {
    if let Some(task) = task {
        crate::navigate::execute_pack_task_partitioned(
            task,
            ctx.root,
            partition_k,
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            out,
        )
    } else {
        let mut stdout_buf = Vec::new();
        crate::commands::communities::execute_communities_query(
            ctx.root,
            None,
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            page,
            &mut stdout_buf,
        )?;
        out.write_all(&stdout_buf)
    }
}

pub fn compute_dmm_from_facts<W: Write>(
    baseline_snap: &crate::gate::baseline::QualitySnapshot,
    working_snap: &crate::gate::baseline::QualitySnapshot,
    head_sha: &str,
    out: &mut W,
) -> io::Result<crate::metrics::DmmReport> {
    let rep =
        crate::metrics::compute_dmm_report(baseline_snap, working_snap, "working-tree", head_sha);
    crate::metrics::serialize_dmm_xml(&rep, out)?;
    Ok(rep)
}

pub fn execute_dmm_query<W: Write>(
    rev_opt: Option<&str>,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<i32> {
    let head_sha = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
    let short_sha = &head_sha[..9.min(head_sha.len())];
    let working_snap = crate::gate::baseline::compute_snapshot(ingest, graph, interner, short_sha);
    let rev = rev_opt.filter(|r| !r.is_empty()).unwrap_or("HEAD");
    let changed_files = get_working_tree_changed_files(root);
    let baseline_snap = if changed_files.is_empty() && rev == "HEAD" {
        working_snap.clone()
    } else {
        let eff_changed = if changed_files.is_empty() {
            ingest
                .files
                .iter()
                .map(|f| interner.resolve(&f.path).to_string())
                .collect()
        } else {
            changed_files
        };
        crate::gate::quality_delta::load_revision_baseline_incremental(
            root,
            rev,
            ingest,
            interner,
            &working_snap,
            &eff_changed,
            short_sha,
        )
    };
    compute_dmm_from_facts(&baseline_snap, &working_snap, &head_sha, out)?;
    Ok(0)
}

pub fn compute_plan_lanes_from_facts(
    ctx: &SafetyContext<'_>,
    requested: usize,
    task: &str,
    brief_lines: Option<&[String]>,
    page: crate::serialize::PageRequest,
) -> serde_json::Value {
    let pr = ctx.graph.page_rank(&crate::rank::PageRankConfig::default());
    let corpus = crate::gate::lanes::LaneCorpus {
        root: ctx.root,
        ingest: ctx.ingest,
        graph: ctx.graph,
        pr: &pr,
        interner: ctx.interner,
    };
    if let Some(lines) = brief_lines {
        crate::gate::lanes::compute_plan_lanes_with_brief(corpus, lines, page)
    } else {
        crate::gate::lanes::compute_plan_lanes(corpus, requested, task, page)
    }
}
