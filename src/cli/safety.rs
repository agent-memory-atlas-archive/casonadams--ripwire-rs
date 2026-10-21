use super::args::TopSafetyArgs;
use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::serialize::{ToXmlString, XmlEscapeExt};
use std::io::{self, Write};
use std::path::Path;
#[derive(Default, Clone, Copy)]
pub struct GateDispatchArgs<'a> {
    pub edit_check: Option<&'a str>,
    pub affected: Option<&'a str>,
    pub quality_baseline: bool,
    pub quality_delta: bool,
    pub scope: Option<&'a str>,
    pub quality_ack: Option<&'a str>,
    pub ack_only: Option<&'a str>,
    pub show_stale: bool,
    pub allow_dirty: bool,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl GateDispatchArgs<'_> {
    pub fn is_active(&self) -> bool {
        self.edit_check.is_some()
            || self.affected.is_some()
            || self.quality_baseline
            || self.quality_delta
    }
}

pub fn execute_gate_action<W: Write>(
    args: GateDispatchArgs<'_>,
    state: &crate::mcp::McpState,
    out: &mut W,
) -> Option<io::Result<()>> {
    if let Some(sym) = args.edit_check {
        let page = args.limit.map(|l| (args.offset.unwrap_or(0), l));
        return Some(execute_edit_check_with(sym, page, state, out));
    }
    if let Some(spec) = args.affected {
        return Some(execute_affected_with(spec, state, out));
    }
    if args.quality_baseline {
        return Some(execute_quality_baseline_with(state, args.allow_dirty, out));
    }
    if args.quality_delta {
        return Some(run_quality_delta_gate_with_args(args, state, out));
    }
    None
}

pub fn run_quality_delta_gate_with_args<W: Write>(
    args: GateDispatchArgs<'_>,
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    match execute_quality_delta_with_args(args, state, out) {
        Ok(has_failures) if has_failures => {
            let _ = out.flush();
            std::process::exit(2);
        }
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

fn load_gate_state(root: &str, is_delta: bool) -> io::Result<crate::mcp::McpState> {
    let state_opt = if is_delta {
        crate::mcp::McpState::load_uncached(root)
    } else {
        crate::mcp::McpState::load(root)
    };
    let mut state = state_opt.ok_or_else(|| io::Error::other("failed to load repository"))?;
    if is_delta && !get_working_tree_changed_files(root).is_empty() {
        let cutoff = crate::git::history::resolve_cutoff_timestamp(None, 1);
        state.commits = crate::git::history::walk_repository_history_scoped(root, cutoff).ok();
    }
    Ok(state)
}

/// Refuse gate modifiers passed without the verb they shape.
pub fn validate_gate_modifiers(args: &GateDispatchArgs<'_>) -> io::Result<()> {
    if args.allow_dirty && !args.quality_baseline {
        return Err(io::Error::other(
            "ripwire-rs: --allow-dirty modifies --quality-baseline; pass it too",
        ));
    }
    if args.show_stale && !args.quality_delta {
        return Err(io::Error::other(
            "ripwire-rs: --show-stale modifies --quality-delta; pass it too",
        ));
    }
    Ok(())
}

pub fn dispatch_gate_command(root: &str, args: GateDispatchArgs<'_>) -> Option<io::Result<()>> {
    if let Err(e) = validate_gate_modifiers(&args) {
        return Some(Err(e));
    }
    if !args.is_active() {
        return None;
    }
    let state = match load_gate_state(root, args.quality_delta) {
        Ok(s) => s,
        Err(e) => return Some(Err(e)),
    };
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
    let res = execute_gate_action(args, &state, &mut stdout);
    let _ = stdout.flush();
    res
}

pub fn execute_with_git_state_fmt<F>(
    root: &str,
    cutoff: Option<i64>,
    pretty: bool,
    json_out: bool,
    runner: F,
) -> io::Result<()>
where
    F: FnOnce(
        &[crate::git::history::CommitInfo],
        &crate::mcp::McpState,
        &mut crate::serialize::XmlWriter<'_, io::StdoutLock<'_>>,
    ) -> io::Result<()>,
{
    let commits_vec = crate::git::history::walk_repository_history_scoped(root, cutoff).ok();
    let commits = commits_vec.as_deref().unwrap_or(&[]);
    let state = crate::mcp::McpState::load(root)
        .ok_or_else(|| io::Error::other("failed to load repository"))?;
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::with_format(&mut stdout_lock, pretty, json_out);
    runner(commits, &state, &mut stdout)?;
    stdout.flush()
}

#[derive(Default, Clone, Copy)]
pub struct GitMetricsFlags<'a> {
    pub cochange: Option<Option<&'a str>>,
    pub cochange_groups: bool,
    pub cochange_recur: Option<u32>,
    pub hotspots: bool,
    pub since: Option<&'a str>,
    pub pretty: bool,
    pub json_out: bool,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub fn dispatch_git_metrics_with_flags(
    root: &str,
    flags: GitMetricsFlags<'_>,
) -> Option<io::Result<()>> {
    if flags.cochange.is_some() || flags.cochange_groups {
        let probed = flags.cochange.flatten();
        let cutoff = crate::git::history::resolve_cutoff_timestamp(flags.since, 18);
        return Some(execute_with_git_state_fmt(
            root,
            cutoff,
            flags.pretty,
            flags.json_out,
            |commits, state, out| execute_cochange_with_options(probed, flags, commits, state, out),
        ));
    }
    if flags.hotspots {
        let cutoff = crate::git::history::resolve_cutoff_timestamp(flags.since, 12);
        return Some(execute_with_git_state_fmt(
            root,
            cutoff,
            flags.pretty,
            flags.json_out,
            |commits, state, out| execute_hotspots_with(flags.since, commits, state, out),
        ));
    }
    None
}

pub fn get_working_tree_changed_files(root: &str) -> Vec<String> {
    crate::git::repo::working_tree_changes(root)
}

fn resolve_changed_or_spec_files(root: &str, files_spec: Option<&str>) -> Vec<String> {
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

pub fn execute_test_gate_query_paged<W: Write>(
    root: &str,
    files_spec: Option<&str>,
    page: crate::serialize::PageRequest,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    use crate::domain::CorpusView;
    use crate::serialize::{GitStampExt, XmlReport};
    let files = resolve_changed_or_spec_files(root, files_spec);
    let specs: Vec<&str> = files.iter().map(String::as_str).collect();
    let mut report = crate::gate::affected::find_affected_tests(&specs, ingest, graph, interner);
    let gates = crate::gate::script_gates::collect_script_gates(root, ingest, interner);
    let gate_rows = gates.covering_rows(&files, &report.tests);
    report.tests.extend(gate_rows);
    crate::gate::attach_test_runners(
        &mut report.tests,
        std::path::Path::new(root),
        ingest,
        interner,
    );
    let tested = crate::navigate::compute_tested_node_mask(ingest, graph, interner);
    let sum = crate::gate::summarize_test_gate(&report, &tested, ingest, interner);
    let view = (ingest, interner);
    let untested: Vec<crate::serialize::arch_xml::UntestedSymbol<'_>> = sum
        .untested
        .iter()
        .map(|&id| {
            let sym = &ingest[id];
            crate::serialize::arch_xml::UntestedSymbol {
                name: view.sym_name(sym),
                path: view.sym_path(sym),
                line: sym.line,
                ccx: sym.ccx,
            }
        })
        .collect();
    let at_stamp = root.git_at_or_head();
    crate::serialize::arch_xml::TestGateReport {
        root,
        at_stamp: &at_stamp,
        changed: files.len(),
        impacted: sum.impacted,
        tests: &report.tests,
        untested: &untested,
        untested_modscope: sum.untested_modscope,
        run_first: sum.run_first,
        script_gates_unmodelled: gates.unmodelled,
        script_gates_registered: gates.registered.len(),
        script_gates_mapped: gates.mapped(),
        page,
        gauge_attrs: &graph.xml_gauge_attrs(),
    }
    .render_envelope(out)?;
    Ok(!report.tests.is_empty() || !sum.untested.is_empty())
}

pub fn format_situ_list<W: Write>(
    out: &mut W,
    title: &str,
    items: Option<&Vec<serde_json::Value>>,
) -> io::Result<()> {
    let empty = Vec::new();
    let list = items.unwrap_or(&empty);
    writeln!(out, "  {title} ({})", list.len())?;
    for item in list {
        writeln!(out, "        {}", item.as_str().unwrap_or(""))?;
    }
    Ok(())
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
    let val = crate::mcp::graph_tools::call_situational_awareness(
        ingest,
        graph,
        interner,
        &files,
        commits.as_deref(),
    );
    writeln!(
        out,
        "ripwire-rs situational-awareness — {} changed file(s)",
        files.len()
    )?;
    writeln!(out, "root: {root}")?;
    writeln!(out, "  [1] blast radius: {} symbols", val["blast_radius"])?;
    format_situ_list(out, "[2] tests to run", val["tests_to_run"].as_array())?;
    format_situ_list(
        out,
        "[3] co-change — usually edited with these",
        val["forgotten_partners"].as_array(),
    )
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

fn validate_merge_scout_branches(root: &str, branches: &str) -> bool {
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

fn execute_merge_scout_dispatch<W: Write>(
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
    let eff_branches = if dirty && !branches.contains(',') {
        format!("{branches},working-tree")
    } else {
        branches.to_string()
    };
    let rep = crate::gate::merge_scout::evaluate_merge_scout(&eff_branches, ingest, interner);
    crate::gate::merge_scout::serialize_merge_scout_xml(&rep, out)?;
    Ok(None)
}

fn execute_pr_context_dispatch<W: Write>(
    root: &str,
    ref_opt: Option<&str>,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let base_ref = ref_opt.filter(|s| !s.is_empty()).unwrap_or("working-tree");
    let changed = get_working_tree_changed_files(root);
    let rep = crate::gate::pr_context::compute_pr_context_in_root(
        root, &changed, ingest, graph, interner, base_ref,
    );
    crate::gate::pr_context::serialize_pr_context_xml(&rep, out)
}

fn execute_scan_skills_dispatch<W: Write>(raw: &str, out: &mut W) -> io::Result<i32> {
    let scan_dir = if !raw.is_empty() {
        crate::skills::resolve_skills_dir(Some(raw))
    } else if let Ok(codex_home) = std::env::var("CODEX_HOME")
        && std::path::Path::new(&codex_home).join("skills").is_dir()
    {
        std::path::Path::new(&codex_home).join("skills")
    } else if std::path::Path::new("skills").is_dir() {
        std::path::PathBuf::from("skills")
    } else {
        crate::skills::default_skills_dir()
    };
    let rep = crate::skills::scan_skills_directory(&scan_dir)?;
    eprintln!("{} skill file(s) scanned", rep.files_scanned);
    crate::skills::serialize_skill_scan_xml(&rep, out)?;
    Ok(match rep.verdict {
        "critical" => 2,
        "warn" => 1,
        _ => 0,
    })
}

fn execute_edit_plan_dispatch<W: Write>(
    plan_spec: &str,
    root: &str,
    dry_run: bool,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<i32> {
    let plan_raw = if plan_spec == "-" {
        let mut buf = String::new();
        io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
        buf
    } else {
        std::fs::read_to_string(plan_spec).unwrap_or_else(|_| plan_spec.to_string())
    };
    let res = crate::mcp::edit::execute_edit_plan_json(root, &plan_raw, dry_run, ingest, interner);
    writeln!(out, "{res}")?;
    Ok(if res.get("error").is_some() { 1 } else { 0 })
}

fn execute_partition_or_baseline_queries<W: Write>(
    s: &TopSafetyArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<Option<i32>> {
    if let Some(part_opt) = s.partition {
        let k = part_opt.unwrap_or(2);
        if let Some(task) = s.task.as_deref() {
            crate::navigate::execute_pack_task_partitioned(
                task, root, k, ingest, graph, interner, out,
            )?;
            return Ok(Some(0));
        }
        let mut stdout_buf = Vec::new();
        crate::cli::queries::execute_communities_query(
            root,
            None,
            ingest,
            graph,
            interner,
            &mut stdout_buf,
        )?;
        out.write_all(&stdout_buf)?;
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

fn execute_dmm_query<W: Write>(
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
    let rep = crate::metrics::compute_dmm_report(
        &baseline_snap,
        &working_snap,
        "working-tree",
        &head_sha,
    );
    crate::metrics::serialize_dmm_xml(&rep, out)?;
    Ok(0)
}

fn execute_tool_safety_queries<W: Write>(
    s: &TopSafetyArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<Option<i32>> {
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
        let pr = graph.page_rank(&crate::rank::PageRankConfig::default());
        let val = if let Some(lines) = brief_lines.as_deref() {
            crate::gate::lanes::compute_plan_lanes_with_brief(
                root, lines, ingest, graph, &pr, interner,
            )
        } else {
            crate::gate::lanes::compute_plan_lanes(
                root, requested, task, ingest, graph, &pr, interner,
            )
        };
        serde_json::to_writer(&mut *out, &val)?;
        writeln!(out)?;
        return Ok(Some(0));
    }
    if let Some(dir_opt) = &s.scan_skills {
        let raw = dir_opt.as_deref().unwrap_or("");
        return execute_scan_skills_dispatch(raw, out).map(Some);
    }
    if let Some(plan_spec) = s.edit_plan.as_deref().or(s.edit_payload.as_deref()) {
        return execute_edit_plan_dispatch(plan_spec, root, s.dry_run, ingest, interner, out)
            .map(Some);
    }
    execute_partition_or_baseline_queries(s, root, ingest, graph, interner, out)
}

fn execute_verification_queries<W: Write>(
    s: &TopSafetyArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<Option<i32>> {
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
    s: &TopSafetyArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<Option<i32>> {
    if let Some(code) = execute_verification_queries(s, root, ingest, graph, interner, out)? {
        if code > 0 {
            return Ok(Some(code));
        }
        return Ok(None);
    }
    if let Some(branches) = s.merge_scout.as_deref() {
        return execute_merge_scout_dispatch(root, branches, ingest, interner, out);
    }
    if let Some(code) = execute_tool_safety_queries(s, root, ingest, graph, interner, out)? {
        if code > 0 {
            return Ok(Some(code));
        }
        return Ok(None);
    }
    Ok(None)
}

pub fn execute_hotspots_with<W: Write>(
    since_str: Option<&str>,
    commits: &[crate::git::history::CommitInfo],
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    let anchor_epoch = commits.first().map(|c| c.timestamp).unwrap_or(0);
    let scope = crate::git::history::parse_since_scope(since_str, anchor_epoch);
    let filtered = crate::git::history::filter_commits_by_scope(commits, &scope);
    let churn_map = crate::git::history::compute_file_churn(
        filtered,
        anchor_epoch,
        crate::git::history::MERGE_BOMB_MAX_FILES,
    );
    let counts: std::collections::HashMap<String, u32> = churn_map
        .into_iter()
        .map(|(path, fc)| (path, fc.commits))
        .collect();
    let report = crate::metrics::compute_hotspots(&state.ingest, &state.interner, &counts)
        .with_root(&state.root);
    report.write_xml(out)
}

pub fn print_cochange_partners<W: Write>(
    file: &str,
    partners: &[crate::git::cochange::CoPartner],
    out: &mut W,
) -> io::Result<()> {
    writeln!(out, "<f path=\"{}\" partners=\"{}\">", file, partners.len())?;
    for p in partners {
        let attr = if !p.dep_capable {
            " dep_capable=\"0\""
        } else if p.surprising {
            " surprising=\"1\""
        } else {
            ""
        };
        writeln!(
            out,
            "  <partner path=\"{}\" together=\"{}\" deg=\"{:.2}\" deg_rev=\"{:.2}\" recur=\"{}\"{} />",
            p.file_path, p.together, p.deg, p.deg_rev, p.recur, attr
        )?;
    }
    writeln!(out, "</f>")
}

fn format_copair_row(p: &crate::git::cochange::CoPair) -> String {
    let surp = usize::from(p.surprising);
    let mut row = format!(
        "  <pair a=\"{}\" b=\"{}\" together=\"{}\" deg=\"{:.2}\" conf_ab=\"{:.2}\" conf_ba=\"{:.2}\" driver=\"{}\" recur=\"{}\" surprising=\"{surp}\"",
        p.file_a.xml_escaped(),
        p.file_b.xml_escaped(),
        p.together,
        p.deg,
        p.conf_ab,
        p.conf_ba,
        p.driver,
        p.recur
    );
    if !p.dep_capable {
        row.push_str(" dep_capable=\"0\"");
    }
    row.push_str("/>");
    row
}

pub fn print_cochange_pairs_paged<W: Write>(
    root: &str,
    pairs: &[crate::git::cochange::CoPair],
    limit: Option<usize>,
    offset: Option<usize>,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::{GitStampExt, PaginateSlice};
    let at_attr = root.git_at_xml_attr();
    let total = pairs.len();
    let (pw, page) = pairs.paginate(30, limit, offset);
    let page_attrs = pw.xml_attrs();
    writeln!(
        out,
        "<cochange schema=\"ripwire-rs.cochange/v1\" pairs=\"{total}\" window=\"18mo@HEAD\" sub_windows=\"3\"{page_attrs} root=\"{root}\"{at_attr}>",
    )?;
    for p in page {
        writeln!(out, "{}", format_copair_row(p))?;
    }
    writeln!(out, "</cochange>")
}

pub fn print_cochange_groups_paged<W: Write>(
    root: &str,
    groups: &[crate::git::cochange::CoGroup],
    limit: Option<usize>,
    offset: Option<usize>,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::{GitStampExt, PaginateSlice};
    let at_attr = root.git_at_xml_attr();
    let total = groups.len();
    let (pw, page) = groups.paginate(30, limit, offset);
    let page_attrs = pw.xml_attrs();
    writeln!(
        out,
        "<cochange-groups schema=\"ripwire-rs.cochange-groups/v1\" groups=\"{total}\" window=\"18mo@HEAD\"{page_attrs} root=\"{root}\"{at_attr}>",
    )?;
    for g in page {
        let partners_joined = g
            .partners
            .iter()
            .map(|p| p.xml_escaped().to_string())
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            out,
            "  <group core=\"{}\" partners=\"{partners_joined}\" size=\"{}\" together=\"{}\" deg=\"{:.2}\"/>",
            g.core.xml_escaped(),
            g.partners.len() + 1,
            g.max_together,
            g.max_deg
        )?;
    }
    writeln!(out, "</cochange-groups>")
}

pub fn execute_cochange_with<W: Write>(
    probed_file: Option<&str>,
    since_str: Option<&str>,
    commits: &[crate::git::history::CommitInfo],
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    execute_cochange_with_options(
        probed_file,
        GitMetricsFlags {
            since: since_str,
            ..Default::default()
        },
        commits,
        state,
        out,
    )
}

pub fn execute_cochange_with_options<W: Write>(
    probed_file: Option<&str>,
    flags: GitMetricsFlags<'_>,
    commits: &[crate::git::history::CommitInfo],
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    let anchor_epoch = commits.first().map(|c| c.timestamp).unwrap_or(0);
    let scope = crate::git::history::parse_since_scope(flags.since, anchor_epoch);
    let filtered = crate::git::history::filter_commits_by_scope(commits, &scope);
    let coupling = crate::git::cochange::StaticCouplingMap::build(
        &state.ingest,
        &state.interner,
        Some(&state.graph),
    );
    let opts = crate::git::cochange::CochangeOptions::default();

    let target_file =
        probed_file.filter(|f| !f.is_empty() && *f != "." && !std::path::Path::new(f).is_dir());
    if let Some(file) = target_file {
        let mut partners = crate::git::cochange::mine_cochange_for_file(
            file,
            filtered,
            Some(&coupling),
            Some(&state.ingest),
            Some(&state.interner),
            &opts,
        );
        if let Some(mr) = flags.cochange_recur {
            partners.retain(|p| p.recur >= mr);
        }
        print_cochange_partners(file, &partners, out)
    } else {
        let mut pairs = crate::git::cochange::mine_repo_cochange_pairs(
            filtered,
            Some(&coupling),
            Some(&state.ingest),
            Some(&state.interner),
            &opts,
        );
        if let Some(mr) = flags.cochange_recur {
            pairs.retain(|p| p.recur >= mr);
        }
        if flags.cochange_groups {
            let groups = crate::git::cochange::cluster_cochange_groups(&pairs);
            print_cochange_groups_paged(&state.root, &groups, flags.limit, flags.offset, out)
        } else {
            print_cochange_pairs_paged(&state.root, &pairs, flags.limit, flags.offset, out)
        }
    }
}

pub fn execute_edit_check_with<W: Write>(
    sym_name: &str,
    page: Option<(usize, usize)>,
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    let ids = crate::navigate::find_symbols_by_name(sym_name, &state.ingest, &state.interner);
    let Some(&sym_id) = ids.first() else {
        writeln!(out, "Symbol '{}' not found", sym_name)?;
        return Ok(());
    };

    let sym = &state.ingest.symbols[sym_id as usize];
    let rel_path = state
        .interner
        .resolve(&state.ingest.files[sym.file_id as usize].path);
    let head_contract =
        crate::gate::contract::load_head_symbol_contract(&state.root, rel_path, sym_name, sym.lang)
            .ok()
            .flatten();

    if let Some(mut report) = crate::gate::edit_check::check_symbol_contract(
        sym_id,
        head_contract.as_ref(),
        &state.ingest,
        &state.graph,
        &state.interner,
    ) {
        if let Some(stamp) = crate::git::history::compute_git_at_stamp(&state.root) {
            report.at_stamp = stamp;
        }
        report.page = page;
        report.write_xml(out)?;
    }
    Ok(())
}

pub fn execute_affected_with<W: Write>(
    specs_str: &str,
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    let specs: Vec<&str> = specs_str.split(',').map(str::trim).collect();
    let mut report = crate::gate::affected::find_affected_tests(
        &specs,
        &state.ingest,
        &state.graph,
        &state.interner,
    );
    crate::gate::attach_test_runners(
        &mut report.tests,
        std::path::Path::new(&state.root),
        &state.ingest,
        &state.interner,
    );
    report.write_xml(out)
}

fn dirty_pin_absorbed_gating(state: &crate::mcp::McpState) -> io::Result<Option<(usize, String)>> {
    if get_working_tree_changed_files(&state.root).is_empty() {
        return Ok(None);
    }
    let rep = crate::gate::quality_delta::evaluate_quality_delta_with_basis(
        &state.root,
        state,
        crate::gate::quality_delta::DeltaBasis::HeadOnly,
    )?;
    let mut gating = rep.regressions.iter().filter(|r| r.is_gating);
    let Some(first) = gating.next() else {
        return Ok(None);
    };
    let first_row = format!(
        "{} {} {}:{} was={} now={}",
        first.kind, first.sym_name, first.file_path, first.line, first.was_val, first.now_val
    );
    Ok(Some((1 + gating.count(), first_row)))
}

pub fn execute_quality_baseline_with<W: Write>(
    state: &crate::mcp::McpState,
    allow_dirty: bool,
    out: &mut W,
) -> io::Result<()> {
    let in_memory = state.root == ":memory:" || state.root.is_empty();
    let head_sha = if in_memory {
        String::new()
    } else {
        crate::git::history::resolve_head_sha(&state.root).unwrap_or_default()
    };
    let stamp_label = if head_sha.is_empty() {
        "no git HEAD"
    } else {
        head_sha.as_str()
    };
    let mut snap = crate::gate::baseline::compute_snapshot(
        &state.ingest,
        &state.graph,
        &state.interner,
        &head_sha,
    );
    if !in_memory && let Some((absorbed, first_row)) = dirty_pin_absorbed_gating(state)? {
        if !allow_dirty {
            return Err(io::Error::other(format!(
                "ripwire-rs: --quality-baseline: this tree already holds {absorbed} gating finding(s) against HEAD; pinning here would absorb them into the floor. First: {first_row}\n  Commit the tree first, or pass --allow-dirty to pin anyway (the sidecar then records absorbed={absorbed})."
            )));
        }
        snap.dirty = true;
        snap.absorbed = absorbed;
    }
    if in_memory {
        writeln!(
            out,
            "ripwire-rs: pinned quality baseline stamped at {stamp_label} -> in-memory"
        )?;
        return Ok(());
    }
    let baseline_path = Path::new(&state.root).join(crate::gate::baseline::DEFAULT_BASELINE_FILE);
    crate::gate::baseline::write_baseline_file(&snap, &baseline_path)?;
    writeln!(
        out,
        "ripwire-rs: pinned quality baseline stamped at {stamp_label} -> {}",
        baseline_path.display()
    )?;
    Ok(())
}
pub fn execute_quality_delta_with_args<W: Write>(
    args: GateDispatchArgs<'_>,
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<bool> {
    let mut report = crate::gate::quality_delta::evaluate_quality_delta(&state.root, state)?;
    crate::gate::quality_delta::apply_cli_delta_options(
        state,
        &mut report,
        crate::gate::quality_delta::DeltaCliOptions {
            scope: args.scope,
            quality_ack: args.quality_ack,
            ack_only: args.ack_only,
            show_stale: args.show_stale,
        },
    )?;
    report.write_xml(out)?;
    Ok(report.has_gating_failures())
}
