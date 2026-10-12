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
    let state = match crate::mcp::McpState::load(root) {
        Some(s) => s,
        None => return Some(Err(io::Error::other("failed to load repository"))),
    };
    let mut stdout = io::stdout().lock();
    execute_gate_action(args, &state, &mut stdout)
}

pub fn execute_with_git_state<F>(root: &str, runner: F) -> io::Result<()>
where
    F: FnOnce(
        &[crate::git::history::CommitInfo],
        &crate::mcp::McpState,
        &mut io::StdoutLock<'_>,
    ) -> io::Result<()>,
{
    let state = crate::mcp::McpState::load(root)
        .ok_or_else(|| io::Error::other("failed to load repository index"))?;
    let commits_vec = crate::git::history::walk_repository_history(Path::new(root)).ok();
    let commits = commits_vec.as_deref().unwrap_or(&[]);
    let mut stdout = io::stdout().lock();
    runner(commits, &state, &mut stdout)
}

pub fn dispatch_git_metrics_command(
    root: &str,
    cochange: Option<Option<&str>>,
    hotspots: bool,
    since: Option<&str>,
) -> Option<io::Result<()>> {
    if let Some(probed) = cochange {
        return Some(execute_with_git_state(root, |commits, state, out| {
            execute_cochange_with(probed, since, commits, state, out)
        }));
    }
    if hotspots {
        return Some(execute_with_git_state(root, |commits, state, out| {
            execute_hotspots_with(since, commits, state, out)
        }));
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
    crate::serialize::serialize_test_gate_xml(root, &report.tests, report.reached_symbols, out)?;
    Ok(report.seed_count > 0 && (!report.tests.is_empty() || report.reached_symbols > 0))
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

pub fn execute_safety_dispatch<W: Write>(
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

pub fn print_cochange_pairs<W: Write>(
    pairs: &[crate::git::cochange::CoPair],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<cochange schema=\"ripwire.cochange/v1\" pairs=\"{}\">",
        pairs.len()
    )?;
    for p in pairs {
        let attr = if !p.dep_capable {
            " dep_capable=\"0\""
        } else if p.surprising {
            " surprising=\"1\""
        } else {
            ""
        };
        writeln!(
            out,
            "  <pair a=\"{}\" b=\"{}\" together=\"{}\" deg=\"{:.2}\" recur=\"{}\"{} />",
            p.file_a, p.file_b, p.together, p.deg, p.recur, attr
        )?;
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

    if let Some(file) = probed_file {
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
        print_cochange_pairs(&pairs, out)
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
