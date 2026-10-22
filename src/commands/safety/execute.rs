use super::SafetyArgs;
use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::query::safety::SafetyContext;
use crate::serialize::ToXmlString;
use std::io::{self, Write};
use std::path::Path;

pub fn get_working_tree_changed_files(root: &str) -> Vec<String> {
    crate::git::repo::working_tree_changes(root)
}

pub fn resolve_changed_or_spec_files(root: &str, files_spec: Option<&str>) -> Vec<String> {
    match files_spec {
        Some(f) => f
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        None => get_working_tree_changed_files(root),
    }
}

pub fn execute_test_gate_query<W: Write>(
    root: &str,
    files_spec: Option<&str>,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    execute_test_gate_query_paged(
        root,
        files_spec,
        crate::serialize::PageRequest::default(),
        ingest,
        graph,
        interner,
        out,
    )
}

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
    out: &mut W,
) -> io::Result<()> {
    crate::query::safety::SituationalReport {
        root: ctx.root,
        files,
        commits,
        context: *ctx,
    }
    .write_xml(out)
}

pub fn execute_situ_query<W: Write>(
    root: &str,
    files_spec: Option<&str>,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let files = resolve_changed_or_spec_files(root, files_spec);
    let commits = crate::git::history::walk_repository_history(root).ok();
    let ctx = SafetyContext {
        root,
        ingest,
        graph,
        interner,
    };
    execute_situ_from_facts(&ctx, &files, commits.as_deref(), out)
}

pub fn execute_safe_delete_query<W: Write>(
    root: &str,
    sym_name: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
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

pub fn validate_merge_scout_branches(root: &str, branches: &str) -> bool {
    for branch in branches.split(',').map(str::trim).filter(|b| !b.is_empty()) {
        let git_ok = gix::open(root).is_ok_and(|repo| {
            repo.rev_parse_single(branch).is_ok() || repo.find_reference(branch).is_ok()
        });
        if !git_ok {
            eprintln!("ripwire-rs: --merge-scout: unknown ref '{branch}'");
            return false;
        }
    }
    true
}

pub fn execute_merge_scout_from_facts<W: Write>(
    branches: &str,
    dirty: bool,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<crate::gate::merge_scout::MergeScoutReport> {
    let eff_branches = if dirty && !branches.contains(',') {
        format!("{branches},working-tree")
    } else {
        branches.to_string()
    };
    let rep = crate::gate::merge_scout::evaluate_merge_scout(&eff_branches, ingest, interner);
    crate::gate::merge_scout::serialize_merge_scout_xml(&rep, out)?;
    Ok(rep)
}

pub fn execute_merge_scout_dispatch<W: Write>(
    root: &str,
    branches: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<Option<i32>> {
    if !validate_merge_scout_branches(root, branches) {
        return Ok(Some(1));
    }
    let dirty = !get_working_tree_changed_files(root).is_empty();
    execute_merge_scout_from_facts(branches, dirty, ingest, interner, out)?;
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
    root: &str,
    ref_opt: Option<&str>,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let base_ref = ref_opt.filter(|s| !s.is_empty()).unwrap_or("working-tree");
    let changed = get_working_tree_changed_files(root);
    let commits = crate::git::history::walk_repository_history(root).unwrap_or_default();
    let ctx = SafetyContext {
        root,
        ingest,
        graph,
        interner,
    };
    let req = crate::gate::pr_context::PrContextRequest {
        changed_files: &changed,
        base_ref,
        commits: &commits,
    };
    execute_pr_context_from_facts(&ctx, &req, out)?;
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
        crate::commands::architecture::execute_communities_query(
            ctx.root,
            None,
            ctx.ingest,
            ctx.graph,
            ctx.interner,
            &mut stdout_buf,
        )?;
        out.write_all(&stdout_buf)
    }
}

pub fn execute_partition_or_baseline_queries<W: Write>(
    s: &SafetyArgs,
    ctx: &SafetyContext<'_>,
    out: &mut W,
) -> io::Result<Option<i32>> {
    let SafetyContext {
        root,
        ingest,
        graph,
        interner,
    } = *ctx;
    if let Some(part_opt) = s.partition {
        let k = part_opt.unwrap_or(2);
        execute_partition_from_facts(ctx, k, s.task.as_deref(), out)?;
        return Ok(Some(0));
    }
    if s.pin_census || s.baseline || s.baseline_update {
        let head_sha = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
        let short_sha = &head_sha[..9.min(head_sha.len())];
        let snap = crate::gate::baseline::compute_snapshot(ingest, graph, interner, short_sha);
        let baseline_path = Path::new(root).join(crate::gate::baseline::DEFAULT_BASELINE_FILE);
        crate::gate::baseline::write_baseline_file(&snap, &baseline_path)?;
        writeln!(
            out,
            "ripwire-rs: pinned quality baseline stamped at {short_sha} -> {}",
            baseline_path.display()
        )?;
        return Ok(Some(0));
    }
    Ok(None)
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

pub fn compute_plan_lanes_from_facts<W: Write>(
    ctx: &SafetyContext<'_>,
    requested: usize,
    task: &str,
    brief_lines: Option<&[String]>,
    out: &mut W,
) -> io::Result<serde_json::Value> {
    let pr = ctx.graph.page_rank(&crate::rank::PageRankConfig::default());
    let val = if let Some(lines) = brief_lines {
        crate::gate::lanes::compute_plan_lanes_with_brief(
            ctx.root,
            lines,
            ctx.ingest,
            ctx.graph,
            &pr,
            ctx.interner,
        )
    } else {
        crate::gate::lanes::compute_plan_lanes(
            ctx.root,
            requested,
            task,
            ctx.ingest,
            ctx.graph,
            &pr,
            ctx.interner,
        )
    };
    serde_json::to_writer(&mut *out, &val)?;
    writeln!(out)?;
    Ok(val)
}

pub fn execute_tool_safety_queries<W: Write>(
    s: &SafetyArgs,
    ctx: &SafetyContext<'_>,
    out: &mut W,
) -> io::Result<Option<i32>> {
    let SafetyContext {
        root,
        ingest,
        graph,
        interner,
    } = *ctx;
    if let Some(range_opt) = &s.dmm {
        return execute_dmm_query(range_opt.as_deref(), root, ingest, graph, interner, out)
            .map(Some);
    }
    if let Some(lanes_opt) = &s.plan_lanes {
        let requested = lanes_opt.unwrap_or(2);
        let task = s.task.as_deref().unwrap_or("general refactoring");
        let brief_lines = s
            .brief
            .as_deref()
            .filter(|p| !p.is_empty())
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|c| c.lines().map(str::to_string).collect::<Vec<_>>());
        compute_plan_lanes_from_facts(ctx, requested, task, brief_lines.as_deref(), out)?;
        return Ok(Some(0));
    }
    if let Some(dir_opt) = &s.scan_skills {
        let raw = dir_opt.as_deref().unwrap_or("");
        return execute_scan_skills_dispatch(raw, out).map(Some);
    }
    if let Some(plan_spec) = s.edit_plan.as_deref().or(s.edit_payload.as_deref()) {
        return execute_edit_plan_dispatch(plan_spec, ctx, s.dry_run, out).map(Some);
    }
    execute_partition_or_baseline_queries(s, ctx, out)
}

pub fn execute_verification_queries<W: Write>(
    s: &SafetyArgs,
    ctx: &SafetyContext<'_>,
    out: &mut W,
) -> io::Result<Option<i32>> {
    let SafetyContext {
        root,
        ingest,
        graph,
        interner,
    } = *ctx;
    if let Some(files_opt) = &s.test_gate {
        let has_failures =
            execute_test_gate_query(root, files_opt.as_deref(), ingest, graph, interner, out)?;
        if has_failures {
            return Ok(Some(4));
        }
    } else if let Some(files_opt) = &s.situ {
        execute_situ_query(root, files_opt.as_deref(), ingest, graph, interner, out)?;
    } else if let Some(sym) = s.safe_delete.as_deref() {
        execute_safe_delete_query(root, sym, ingest, graph, interner, out)?;
    } else if let Some(testfile) = s.exercises.as_deref() {
        execute_exercises_query(root, testfile, ingest, graph, interner, out)?;
    } else if s.seams {
        use crate::serialize::XmlReport;
        let rep = crate::gate::seams::find_integration_seams(ingest, graph, interner);
        rep.write_xml_report(root, out)?;
    } else if let Some(ref_opt) = &s.pr_context {
        execute_pr_context_dispatch(root, ref_opt.as_deref(), ingest, graph, interner, out)?;
    } else {
        return Ok(None);
    }
    Ok(Some(0))
}

pub fn execute_safety_dispatch<W: Write>(
    s: &SafetyArgs,
    ctx: &SafetyContext<'_>,
    out: &mut W,
) -> io::Result<Option<i32>> {
    if let Some(code) = execute_verification_queries(s, ctx, out)? {
        return Ok((code > 0).then_some(code));
    }
    if let Some(branches) = s.merge_scout.as_deref() {
        return execute_merge_scout_dispatch(ctx.root, branches, ctx.ingest, ctx.interner, out);
    }
    Ok(execute_tool_safety_queries(s, ctx, out)?.filter(|code| *code > 0))
}


