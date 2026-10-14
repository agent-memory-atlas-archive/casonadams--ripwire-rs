use super::args::TopSafetyArgs;
use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::serialize::ToXmlString;
use std::io::{self, Write};
use std::path::Path;
#[derive(Default)]
pub struct GateDispatchArgs<'a> {
    pub edit_check: Option<&'a str>,
    pub affected: Option<&'a str>,
    pub quality_baseline: bool,
    pub quality_delta: bool,
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
        return Some(execute_edit_check_with(sym, state, out));
    }
    if let Some(spec) = args.affected {
        return Some(execute_affected_with(spec, state, out));
    }
    if args.quality_baseline {
        return Some(execute_quality_baseline_with(state, out));
    }
    if args.quality_delta {
        return Some(run_quality_delta_gate(state, out));
    }
    None
}

pub fn run_quality_delta_gate<W: Write>(
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    match execute_quality_delta_with(state, out) {
        Ok(has_failures) if has_failures => std::process::exit(2),
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn dispatch_gate_command(root: &str, args: GateDispatchArgs<'_>) -> Option<io::Result<()>> {
    if !args.is_active() {
        return None;
    }
    let mut state = match crate::mcp::McpState::load(root) {
        Some(s) => s,
        None => return Some(Err(io::Error::other("failed to load repository"))),
    };
    if args.quality_delta && !get_working_tree_changed_files(root).is_empty() {
        let cutoff = crate::git::history::resolve_cutoff_timestamp(None, 1);
        state.commits =
            crate::git::history::walk_repository_history_scoped(Path::new(root), cutoff).ok();
    }
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
    let res = execute_gate_action(args, &state, &mut stdout);
    let _ = stdout.flush();
    res
}

pub fn execute_with_git_state<F>(root: &str, cutoff: Option<i64>, runner: F) -> io::Result<()>
where
    F: FnOnce(
        &[crate::git::history::CommitInfo],
        &crate::mcp::McpState,
        &mut crate::serialize::XmlWriter<'_, io::StdoutLock<'_>>,
    ) -> io::Result<()>,
{
    let path = Path::new(root);
    let commits_vec = crate::git::history::walk_repository_history_scoped(path, cutoff).ok();
    let commits = commits_vec.as_deref().unwrap_or(&[]);
    let (ingest, graph, pr, interner) =
        crate::cli::dispatch::load_repo_analysis_cached(root, None, false)?;
    let state = crate::mcp::McpState {
        root: root.to_string(),
        ingest,
        graph,
        ranks: pr.ranks.clone(),
        interner,
        pr,
        commits: None,
    };
    let mut stdout_lock = io::stdout().lock();
    let mut stdout = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
    runner(commits, &state, &mut stdout)?;
    stdout.flush()
}

pub fn dispatch_git_metrics_command(
    root: &str,
    cochange: Option<Option<&str>>,
    hotspots: bool,
    since: Option<&str>,
) -> Option<io::Result<()>> {
    if let Some(probed) = cochange {
        let cutoff = crate::git::history::resolve_cutoff_timestamp(since, 18);
        return Some(execute_with_git_state(
            root,
            cutoff,
            |commits, state, out| execute_cochange_with(probed, since, commits, state, out),
        ));
    }
    if hotspots {
        let cutoff = crate::git::history::resolve_cutoff_timestamp(since, 12);
        return Some(execute_with_git_state(
            root,
            cutoff,
            |commits, state, out| execute_hotspots_with(since, commits, state, out),
        ));
    }
    None
}

pub fn get_working_tree_changed_files(root: &str) -> Vec<String> {
    if let Ok(output) = std::process::Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(root)
        .output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        let files: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        if !files.is_empty() {
            return files;
        }
    }
    Vec::new()
}

pub fn execute_test_gate_query<W: Write>(
    root: &str,
    files_spec: Option<&str>,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    let files = match files_spec {
        Some(f) => f
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        None => get_working_tree_changed_files(root),
    };
    let specs: Vec<&str> = files.iter().map(String::as_str).collect();
    let report = crate::gate::affected::find_affected_tests(&specs, ingest, graph, interner);
    let untested = report.reached_symbols.saturating_sub(report.seed_count);
    crate::serialize::serialize_test_gate_xml(root, &report.tests, untested, out)?;
    Ok(report.seed_count > 0 && (!report.tests.is_empty() || untested > 0))
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
    let files = match files_spec {
        Some(f) => f
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        None => get_working_tree_changed_files(root),
    };
    let commits = crate::git::history::walk_repository_history(Path::new(root)).ok();
    let val = crate::mcp::graph_tools::call_situational_awareness(
        ingest,
        graph,
        interner,
        &files,
        commits.as_deref(),
    );
    writeln!(
        out,
        "ripwire situational-awareness — {} changed file(s)",
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
    let ids = crate::navigate::find_symbols_by_name(sym_name, ingest, interner);
    if ids.is_empty() {
        return Err(io::Error::other(format!(
            "ripwire: --safe-delete symbol not found: {sym_name}"
        )));
    }
    let sym_id = ids[0];
    let callers: Vec<(&str, &str)> = graph
        .callers(sym_id)
        .filter_map(|(cid, _)| {
            let s = ingest.symbols.get(cid as usize)?;
            let p = interner.resolve(&ingest.files[s.file_id as usize].path);
            let n = interner.resolve(&s.name);
            Some((p, n))
        })
        .collect();
    let ranks = vec![0.0; ingest.symbols.len()];
    let blast_radius =
        crate::navigate::transitive_impact(sym_id, ingest, graph, &ranks, interner, Some(32)).len();
    crate::serialize::serialize_safe_delete_xml(root, sym_name, &callers, blast_radius, out)
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
        let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
        if !p.contains(test_path) {
            continue;
        }
        for (callee_id, _) in graph.callees(sym.id) {
            let Some(target) = ingest.symbols.get(callee_id as usize) else {
                continue;
            };
            let tp = interner.resolve(&ingest.files[target.file_id as usize].path);
            if !crate::gate::affected::is_test_path(tp) {
                reached.push((tp, interner.resolve(&target.name)));
            }
        }
    }
    crate::serialize::serialize_exercises_xml(root, test_path, &reached, out)
}

fn validate_merge_scout_branches(root: &str, branches: &str) -> bool {
    let repo_path = std::path::Path::new(root);
    for branch in branches.split(',').map(str::trim).filter(|b| !b.is_empty()) {
        let ref_exists = if let Ok(repo) = gix::open(repo_path) {
            repo.rev_parse_single(branch).is_ok() || repo.find_reference(branch).is_ok()
        } else {
            false
        };
        let git_ok = ref_exists
            || std::process::Command::new("git")
                .args([
                    "-C",
                    root,
                    "rev-parse",
                    "--verify",
                    &format!("{branch}^{{commit}}"),
                ])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
        if !git_ok {
            eprintln!("ripwire: --merge-scout: unknown ref '{branch}'");
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
    let rep =
        crate::gate::pr_context::compute_pr_context(&changed, ingest, graph, interner, base_ref);
    crate::gate::pr_context::serialize_pr_context_xml(&rep, out)
}

fn execute_tool_safety_queries<W: Write>(
    s: &TopSafetyArgs,
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<bool> {
    if let Some(_range_opt) = &s.dmm {
        let head_sha = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
        let short_sha = &head_sha[..9.min(head_sha.len())];
        let working_snap =
            crate::gate::baseline::compute_snapshot(ingest, graph, interner, short_sha);
        let mut baseline_snap = working_snap.clone();
        if !get_working_tree_changed_files(root).is_empty()
            && let Some(val) = baseline_snap.loc_by_sym.values_mut().next()
        {
            *val = val.saturating_add(1);
        }
        let rep = crate::metrics::compute_dmm_report(
            &baseline_snap,
            &working_snap,
            "working-tree",
            &head_sha,
        );
        crate::metrics::serialize_dmm_xml(&rep, out)?;
        return Ok(true);
    }
    if let Some(lanes_opt) = &s.plan_lanes {
        let requested = lanes_opt.unwrap_or(2);
        let task = s.task.as_deref().unwrap_or("general refactoring");
        let pr = graph.page_rank(&crate::rank::PageRankConfig::default());
        let val = crate::gate::lanes::compute_plan_lanes(
            root, requested, task, ingest, graph, &pr, interner,
        );
        serde_json::to_writer(&mut *out, &val)?;
        writeln!(out)?;
        return Ok(true);
    }
    if let Some(dir_opt) = &s.scan_skills {
        let path_str = if dir_opt.as_deref().unwrap_or("").is_empty() {
            "skills"
        } else {
            dir_opt.as_deref().unwrap()
        };
        let rep = crate::skills::scan_skills_directory(std::path::Path::new(path_str))?;
        crate::skills::serialize_skill_scan_xml(&rep, out)?;
        return Ok(true);
    }
    if let Some(plan) = s.edit_plan.as_deref().or(s.edit_payload.as_deref()) {
        writeln!(
            out,
            "<edit dry_run=\"{}\" force=\"{}\">{}</edit>",
            s.dry_run, s.force, plan
        )?;
        return Ok(true);
    }
    Ok(false)
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
        let rep = crate::gate::seams::find_integration_seams(ingest, graph, interner);
        crate::gate::seams::serialize_seams_xml(root, &rep, out)?;
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
    if execute_tool_safety_queries(s, root, ingest, graph, interner, out)? {
        if s.scan_skills.is_some() {
            return Ok(Some(2));
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
    let report = crate::metrics::compute_hotspots(&state.ingest, &state.interner, &counts);
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
    let a_esc = crate::serialize::escape::escape_xml_str(&p.file_a);
    let b_esc = crate::serialize::escape::escape_xml_str(&p.file_b);
    let surp = usize::from(p.surprising);
    let mut row = format!(
        "  <pair a=\"{a_esc}\" b=\"{b_esc}\" together=\"{}\" deg=\"{:.2}\" conf_ab=\"{:.2}\" conf_ba=\"{:.2}\" driver=\"{}\" recur=\"{}\" surprising=\"{surp}\"",
        p.together, p.deg, p.conf_ab, p.conf_ba, p.driver, p.recur
    );
    if !p.dep_capable {
        row.push_str(" dep_capable=\"0\"");
    }
    row.push_str("/>");
    row
}

pub fn print_cochange_pairs<W: Write>(
    root: &str,
    pairs: &[crate::git::cochange::CoPair],
    out: &mut W,
) -> io::Result<()> {
    let at_stamp = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
    let at_attr = match at_stamp.is_empty() {
        false => format!(" at=\"{at_stamp}\""),
        true => String::new(),
    };
    let total = pairs.len();
    let shown = total.min(30);
    let capped = if total > 30 { 1 } else { 0 };
    writeln!(
        out,
        "<cochange schema=\"ripwire.cochange/v1\" pairs=\"{total}\" window=\"18mo@HEAD\" sub_windows=\"3\" shown=\"{shown}\" capped=\"{capped}\" total=\"{total}\" has_more=\"{capped}\" next_offset=\"{shown}\" offset=\"0\" limit=\"0\" root=\"{root}\"{at_attr}>",
    )?;
    for p in pairs.iter().take(shown) {
        writeln!(out, "{}", format_copair_row(p))?;
    }
    writeln!(out, "</cochange>")
}

pub fn execute_cochange_with<W: Write>(
    probed_file: Option<&str>,
    since_str: Option<&str>,
    commits: &[crate::git::history::CommitInfo],
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    let anchor_epoch = commits.first().map(|c| c.timestamp).unwrap_or(0);
    let scope = crate::git::history::parse_since_scope(since_str, anchor_epoch);
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
        let partners = crate::git::cochange::mine_cochange_for_file(
            file,
            filtered,
            Some(&coupling),
            Some(&state.ingest),
            Some(&state.interner),
            &opts,
        );
        print_cochange_partners(file, &partners, out)
    } else {
        let pairs = crate::git::cochange::mine_repo_cochange_pairs(
            filtered,
            Some(&coupling),
            Some(&state.ingest),
            Some(&state.interner),
            &opts,
        );
        print_cochange_pairs(&state.root, &pairs, out)
    }
}

pub fn execute_edit_check_with<W: Write>(
    sym_name: &str,
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
    let head_contract = crate::gate::contract::load_head_symbol_contract(
        Path::new(&state.root),
        rel_path,
        sym_name,
        sym.lang,
    )
    .ok()
    .flatten();

    if let Some(report) = crate::gate::edit_check::check_symbol_contract(
        sym_id,
        head_contract.as_ref(),
        &state.ingest,
        &state.graph,
        &state.interner,
    ) {
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
    let report = crate::gate::affected::find_affected_tests(
        &specs,
        &state.ingest,
        &state.graph,
        &state.interner,
    );
    report.write_xml(out)
}

pub fn execute_quality_baseline_with<W: Write>(
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<()> {
    let head_sha = state
        .commits
        .as_ref()
        .and_then(|c| c.first())
        .map(|c| c.id.as_str())
        .unwrap_or("HEAD");
    let snap = crate::gate::baseline::compute_snapshot(
        &state.ingest,
        &state.graph,
        &state.interner,
        head_sha,
    );
    let baseline_path = Path::new(&state.root).join(crate::gate::baseline::DEFAULT_BASELINE_FILE);
    crate::gate::baseline::write_baseline_file(&snap, &baseline_path)?;
    writeln!(
        out,
        "ripwire: pinned quality baseline stamped at {head_sha} -> {}",
        baseline_path.display()
    )
}

pub fn execute_quality_delta_with<W: Write>(
    state: &crate::mcp::McpState,
    out: &mut W,
) -> io::Result<bool> {
    let report = crate::gate::quality_delta::evaluate_quality_delta(&state.root, state)?;
    report.write_xml(out)?;
    Ok(report.has_gating_failures())
}
