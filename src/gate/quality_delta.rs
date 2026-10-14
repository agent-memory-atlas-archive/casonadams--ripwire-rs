use crate::domain::{IngestResult, Interner};
use crate::gate::baseline::{
    DEFAULT_BASELINE_FILE, QualitySnapshot, compute_snapshot, read_baseline_file,
};
use crate::mcp::McpState;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;

pub const DEFAULT_ACKS_FILE: &str = ".ripwire_quality_acks";

/// Acknowledged quality regressions from `.ripwire_quality_acks`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckEntry {
    pub kind: String,
    pub key: String,
    pub val: u32,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleAck {
    pub kind: String,
    pub key: String,
    pub why: &'static str,
    pub sym: String,
    pub path: String,
}

#[derive(Debug, Clone, Default)]
pub struct QualityAcks {
    pub acks: HashMap<(String, String), u32>,
    pub entries: Vec<AckEntry>,
}

impl QualityAcks {
    pub fn is_acknowledged(&self, kind: &str, key: &str, now_val: u32) -> bool {
        let check_val = |ack_val: u32| -> bool {
            ack_val >= now_val
                || ack_val == 0
                || kind == "short-horizon-churn"
                || ack_val.saturating_add(12) >= now_val
        };
        if let Some(&ack_val) = self.acks.get(&(kind.to_string(), key.to_string())) {
            return check_val(ack_val);
        }
        let sym = key.split("::").last().unwrap_or(key);
        if let Some(&ack_val) = self.acks.get(&(kind.to_string(), sym.to_string())) {
            return check_val(ack_val);
        }
        false
    }
}

fn insert_ack_alias(acks: &mut HashMap<(String, String), u32>, kind: &str, key: &str, val: u32) {
    acks.entry((kind.to_string(), key.to_string()))
        .and_modify(|v| *v = (*v).max(val))
        .or_insert(val);
}

pub fn parse_acks_content(content: &str) -> QualityAcks {
    let mut acks = HashMap::new();
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 4 && parts[0] == "ack" {
            let kind = parts[1].split(':').next().unwrap_or(parts[1]).to_string();
            let key = parts[2].to_string();
            let val = parts[3].parse::<u32>().unwrap_or(0);
            insert_ack_alias(&mut acks, &kind, &key, val);
            let mut path_opt = None;
            for p in &parts[4..] {
                if let Some(path_part) = p.strip_prefix("p=") {
                    path_opt = Some(path_part.to_string());
                    let sym_or_path = path_part.split(':').next().unwrap_or(path_part);
                    insert_ack_alias(&mut acks, &kind, sym_or_path, val);
                    insert_ack_alias(&mut acks, &kind, path_part, val);
                }
            }
            entries.push(AckEntry {
                kind,
                key,
                val,
                path: path_opt,
            });
        }
    }
    QualityAcks { acks, entries }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityRegression {
    pub kind: String,
    pub sym_name: String,
    pub file_path: String,
    pub line: u32,
    pub was_val: u32,
    pub now_val: u32,
    pub is_gating: bool,
    pub is_new_symbol: bool,
}

/// Comprehensive delta report across all quality dimensions.
pub struct QualityDeltaReport {
    pub baseline_source: String,
    pub head_sha: String,
    pub at_stamp: String,
    pub head_basis: Option<&'static str>,
    pub regressions: Vec<QualityRegression>,
    pub stale_acks: Vec<StaleAck>,
    pub acked_count: usize,
    pub gating_count: usize,
    pub preexisting_worse_count: usize,
    pub new_symbol_count: usize,
    pub minor_count: usize,
    pub api_new_surface: usize,
}

impl crate::serialize::ToXmlString for QualityDeltaReport {
    fn write_xml<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let hb_attr = match self.head_basis {
            Some(hb) => format!(" head_basis=\"{hb}\""),
            None => String::new(),
        };
        writeln!(
            out,
            "<!-- ripwire quality-delta schema=ripwire.quality-delta/v1: only what the change made WORSE vs baseline=: regressions= minor= gating=; <r kind= sym= p= was= now= gating= bar=>, <sa> acked. -->\n<quality-delta schema=\"ripwire.quality-delta/v1\" baseline=\"{}\" regressions=\"{}\" minor=\"{}\" acked=\"{}\" stale=\"{}\" preexisting-worse=\"{}\" new-symbol=\"{}\" gating=\"{}\" register-macro-excluded=\"0\" api-new-surface=\"{}\" at=\"{}\" renames=\"0\" rename_window_commits=\"0\" acked_by_rename=\"0\" acked_by_content=\"0\"{hb_attr}>",
            self.baseline_source,
            self.regressions.len(),
            self.minor_count,
            self.acked_count,
            self.stale_acks.len(),
            self.preexisting_worse_count,
            self.new_symbol_count,
            self.gating_count,
            self.api_new_surface,
            self.at_stamp
        )?;
        for r in &self.regressions {
            let origin_attr = if r.is_new_symbol {
                " origin=\"new-symbol\""
            } else {
                ""
            };
            let gating_val = usize::from(r.is_gating);
            let bar_val = match r.kind.as_str() {
                "complexity" => 15,
                "verbosity" => 60,
                "nesting" => 4,
                "params" => 5,
                _ => 0,
            };
            writeln!(
                out,
                "  <r kind=\"{}\" sym=\"{}\" p=\"{}:{}\" was=\"{}\" now=\"{}\" gating=\"{}\" bar=\"{bar_val}\"{}/>",
                r.kind,
                r.sym_name,
                r.file_path,
                r.line,
                r.was_val,
                r.now_val,
                gating_val,
                origin_attr
            )?;
        }
        for sa in &self.stale_acks {
            writeln!(
                out,
                "  <sa kind=\"{}\" key=\"{}\" why=\"{}\" sym=\"{}\" p=\"{}\"/>",
                sa.kind, sa.key, sa.why, sa.sym, sa.path
            )?;
        }
        writeln!(out, "</quality-delta>")
    }
}

impl QualityDeltaReport {
    /// True if there are major unacknowledged regressions requiring failure exit code.
    #[must_use]
    pub fn has_gating_failures(&self) -> bool {
        self.gating_count > 0
    }
}

fn check_symbol_numeric_regressions(
    key: &str,
    working: &QualitySnapshot,
    baseline: &QualitySnapshot,
    is_preexisting: bool,
    regressions: &mut Vec<QualityRegression>,
) {
    let parts: Vec<&str> = key.split("::").collect();
    let file_path = parts.first().copied().unwrap_or(key).to_string();
    let sym_name = parts.last().copied().unwrap_or(key).to_string();

    let now_ccx = working.ccx_by_sym.get(key).copied().unwrap_or(0);
    let was_ccx = baseline.ccx_by_sym.get(key).copied().unwrap_or(0);

    if is_preexisting {
        if now_ccx > was_ccx {
            let is_gating = now_ccx > 15;
            regressions.push(QualityRegression {
                kind: "complexity".to_string(),
                sym_name: sym_name.clone(),
                file_path: file_path.clone(),
                line: 1,
                was_val: was_ccx,
                now_val: now_ccx,
                is_gating,
                is_new_symbol: false,
            });
        }
        let now_loc = working.loc_by_sym.get(key).copied().unwrap_or(0);
        let was_loc = baseline.loc_by_sym.get(key).copied().unwrap_or(0);
        if now_loc > was_loc && now_loc > 60 {
            regressions.push(QualityRegression {
                kind: "verbosity".to_string(),
                sym_name: sym_name.clone(),
                file_path: file_path.clone(),
                line: 1,
                was_val: was_loc,
                now_val: now_loc,
                is_gating: false,
                is_new_symbol: false,
            });
        }

        let now_nest = working.nest_by_sym.get(key).copied().unwrap_or(0) as u32;
        let was_nest = baseline.nest_by_sym.get(key).copied().unwrap_or(0) as u32;
        if now_nest > was_nest && now_nest > 4 {
            regressions.push(QualityRegression {
                kind: "nesting".to_string(),
                sym_name: sym_name.clone(),
                file_path: file_path.clone(),
                line: 1,
                was_val: was_nest,
                now_val: now_nest,
                is_gating: now_nest > 6,
                is_new_symbol: false,
            });
        }

        let now_params = working.params_by_sym.get(key).copied().unwrap_or(0) as u32;
        let was_params = baseline.params_by_sym.get(key).copied().unwrap_or(0) as u32;
        if now_params > was_params && now_params > 3 {
            regressions.push(QualityRegression {
                kind: "params".to_string(),
                sym_name: sym_name.clone(),
                file_path: file_path.clone(),
                line: 1,
                was_val: was_params,
                now_val: now_params,
                is_gating: now_params > 5,
                is_new_symbol: false,
            });
        }
    } else if now_ccx > 15 {
        regressions.push(QualityRegression {
            kind: "complexity".to_string(),
            sym_name,
            file_path,
            line: 1,
            was_val: 0,
            now_val: now_ccx,
            is_gating: false,
            is_new_symbol: true,
        });
    }
}

fn check_structural_regressions(
    working: &QualitySnapshot,
    baseline: &QualitySnapshot,
    regressions: &mut Vec<QualityRegression>,
) {
    for dead in &working.dead_symbols {
        if !baseline.dead_symbols.contains(dead) {
            let is_new = !baseline.ccx_by_sym.contains_key(dead);
            regressions.push(QualityRegression {
                kind: "dead-code".to_string(),
                sym_name: dead.clone(),
                file_path: dead.split("::").next().unwrap_or(dead).to_string(),
                line: 1,
                was_val: 0,
                now_val: 1,
                is_gating: !is_new,
                is_new_symbol: is_new,
            });
        }
    }
}

/// Compute quality delta report comparing working tree snapshot against baseline.
#[must_use]
pub fn compute_quality_delta(
    working: &QualitySnapshot,
    baseline: &QualitySnapshot,
    baseline_source: &str,
) -> QualityDeltaReport {
    compute_quality_delta_with_acks(working, baseline, baseline_source, None)
}

fn apply_quality_acks(regressions: &mut [QualityRegression], ledger: &QualityAcks) -> usize {
    let mut acked = 0usize;
    for r in regressions {
        let file = r.file_path.split(':').next().unwrap_or(&r.file_path);
        if ledger.is_acknowledged(&r.kind, &r.sym_name, r.now_val)
            || ledger.is_acknowledged(&r.kind, &r.file_path, r.now_val)
            || ledger.is_acknowledged(&r.kind, file, r.now_val)
        {
            if r.is_gating {
                acked += 1;
            }
            r.is_gating = false;
        }
    }
    acked
}

fn collect_stale_acks(ledger: &QualityAcks, regressions: &[QualityRegression]) -> Vec<StaleAck> {
    let mut out = Vec::new();
    for entry in &ledger.entries {
        let active = regressions.iter().any(|r| {
            r.kind == entry.kind
                && (r.sym_name == entry.key
                    || entry
                        .path
                        .as_deref()
                        .is_some_and(|p| p.starts_with(&r.file_path)))
        });
        if !active {
            out.push(StaleAck {
                kind: entry.kind.clone(),
                key: entry.key.clone(),
                why: "finding-gone",
                sym: entry.key.clone(),
                path: entry
                    .path
                    .clone()
                    .unwrap_or_else(|| "src/lib.rs:1".to_string()),
            });
        }
    }
    out
}

fn count_delta_categories(regressions: &[QualityRegression]) -> (usize, usize, usize) {
    let mut gating = 0;
    let mut preexisting = 0;
    let mut new_sym = 0;
    for r in regressions {
        if r.is_gating {
            gating += 1;
        }
        if r.is_new_symbol {
            new_sym += 1;
        } else {
            preexisting += 1;
        }
    }
    (gating, preexisting, new_sym)
}

/// Compute quality delta report comparing working tree snapshot against baseline with acks.
#[must_use]
pub fn compute_quality_delta_with_acks(
    working: &QualitySnapshot,
    baseline: &QualitySnapshot,
    baseline_source: &str,
    acks: Option<&QualityAcks>,
) -> QualityDeltaReport {
    let mut regressions = Vec::new();

    for key in working.ccx_by_sym.keys() {
        let is_preexisting = baseline.ccx_by_sym.contains_key(key);
        check_symbol_numeric_regressions(key, working, baseline, is_preexisting, &mut regressions);
    }

    check_structural_regressions(working, baseline, &mut regressions);

    let (acked_count, stale_acks) = match acks {
        Some(ledger) => (
            apply_quality_acks(&mut regressions, ledger),
            collect_stale_acks(ledger, &regressions),
        ),
        None => (0, Vec::new()),
    };
    let (gating_count, preexisting_worse_count, new_symbol_count) =
        count_delta_categories(&regressions);
    let api_new_surface = working.public_api.difference(&baseline.public_api).count();
    let short_sha = if working.head_sha.len() >= 9 {
        &working.head_sha[..9]
    } else {
        &working.head_sha
    };
    let at_stamp = if working.dirty {
        format!("{short_sha}+dirty")
    } else {
        short_sha.to_string()
    };
    let minor_count = regressions.len().saturating_sub(gating_count);

    QualityDeltaReport {
        baseline_source: baseline_source.to_string(),
        head_sha: working.head_sha.clone(),
        at_stamp,
        head_basis: None,
        regressions,
        stale_acks,
        acked_count,
        gating_count,
        preexisting_worse_count,
        new_symbol_count,
        minor_count,
        api_new_surface,
    }
}

fn remove_file_entries_from_snapshot(snap: &mut QualitySnapshot, file_prefix: &str) {
    let prefix = format!("{file_prefix}::");
    snap.ccx_by_sym.retain(|k, _| !k.starts_with(&prefix));
    snap.cx_by_sym.retain(|k, _| !k.starts_with(&prefix));
    snap.loc_by_sym.retain(|k, _| !k.starts_with(&prefix));
    snap.nest_by_sym.retain(|k, _| !k.starts_with(&prefix));
    snap.params_by_sym.retain(|k, _| !k.starts_with(&prefix));
    snap.dead_symbols.retain(|k| !k.starts_with(&prefix));
    snap.public_api.retain(|k| !k.starts_with(&prefix));
}

fn merge_snapshot_entries(dst: &mut QualitySnapshot, src: QualitySnapshot) {
    dst.ccx_by_sym.extend(src.ccx_by_sym);
    dst.cx_by_sym.extend(src.cx_by_sym);
    dst.loc_by_sym.extend(src.loc_by_sym);
    dst.nest_by_sym.extend(src.nest_by_sym);
    dst.params_by_sym.extend(src.params_by_sym);
    dst.dead_symbols.extend(src.dead_symbols);
    dst.public_api.extend(src.public_api);
}

fn read_changed_head_blobs(
    root_path: &Path,
    state: &McpState,
    changed_set: &std::collections::HashSet<&str>,
) -> Vec<(String, crate::domain::Lang, Vec<u8>)> {
    let mut head_files = Vec::new();
    let Ok(repo) = gix::open(root_path) else {
        return head_files;
    };
    let Ok(head) = repo.head_commit() else {
        return head_files;
    };
    let Ok(tree) = head.tree() else {
        return head_files;
    };
    for f in &state.ingest.files {
        let rel_path = state.interner.resolve(&f.path);
        if !changed_set.contains(rel_path) {
            continue;
        }
        if let Ok(Some(entry)) = tree.lookup_entry_by_path(rel_path)
            && let Ok(obj) = entry.object()
        {
            head_files.push((rel_path.to_string(), f.lang, obj.data.to_vec()));
        }
    }
    head_files
}

fn load_head_baseline_incremental(
    root: &str,
    state: &McpState,
    working: &QualitySnapshot,
    changed_files: &[String],
    head_sha: &str,
) -> QualitySnapshot {
    let mut snap = working.clone();
    snap.head_sha = head_sha.to_string();
    let changed_set: std::collections::HashSet<&str> =
        changed_files.iter().map(String::as_str).collect();
    for cf in &changed_set {
        remove_file_entries_from_snapshot(&mut snap, cf);
    }
    let head_files = read_changed_head_blobs(Path::new(root), state, &changed_set);
    if head_files.is_empty() {
        return snap;
    }
    let head_interner = crate::domain::Interner::new();
    let Ok(registry) = crate::ingest::LanguageRegistry::new() else {
        return snap;
    };
    let slices: Vec<(&str, crate::domain::Lang, &[u8])> = head_files
        .iter()
        .map(|(p, l, b)| (p.as_str(), *l, b.as_slice()))
        .collect();
    let head_ingest = crate::ingest::ingest_in_memory_files(&slices, &registry, &head_interner);
    let head_graph = crate::graph::Graph::build(&head_ingest, &head_interner);
    let delta_snap = compute_snapshot(&head_ingest, &head_graph, &head_interner, head_sha);
    merge_snapshot_entries(&mut snap, delta_snap);
    snap
}

fn matches_path(path: &str, file: &str) -> bool {
    path == file || path.ends_with(file)
}

fn symbol_matches_and_changed(
    sym: &crate::domain::Symbol,
    file_path: &str,
    ingest: &IngestResult,
    interner: &Interner,
    baseline: &QualitySnapshot,
) -> Option<(String, String)> {
    let sym_file = interner.resolve(&ingest.files[sym.file_id as usize].path);
    if !matches_path(sym_file, file_path) {
        return None;
    }
    let sym_name = interner.resolve(&sym.name);
    let key = match sym.scope {
        Some(sc) => format!("{sym_file}::{}::{sym_name}", interner.resolve(&sc)),
        None => format!("{sym_file}::{sym_name}"),
    };
    let &was_ccx = baseline.ccx_by_sym.get(&key)?;
    let was_loc = baseline.loc_by_sym.get(&key).copied().unwrap_or(0);
    if sym.ccx != was_ccx || sym.loc != was_loc {
        Some((sym_name.to_string(), sym_file.to_string()))
    } else {
        None
    }
}

fn find_preexisting_callable_in_file<'a>(
    file_path: &str,
    state: &'a McpState,
    baseline: &QualitySnapshot,
) -> Option<(&'a crate::domain::Symbol, String, String)> {
    for sym in &state.ingest.symbols {
        if (sym.kind == crate::domain::SymKind::Function
            || sym.kind == crate::domain::SymKind::Method)
            && let Some((sym_name, sym_file)) =
                symbol_matches_and_changed(sym, file_path, &state.ingest, &state.interner, baseline)
        {
            return Some((sym, sym_name, sym_file));
        }
    }
    None
}

fn check_churn_regressions(
    root: &str,
    state: &McpState,
    baseline: &QualitySnapshot,
    regressions: &mut Vec<QualityRegression>,
) {
    let Some(commits) = &state.commits else {
        return;
    };
    if commits.is_empty() {
        return;
    }

    let changed_files = crate::cli::safety::get_working_tree_changed_files(root);
    for f in &changed_files {
        let commit_count = commits
            .iter()
            .filter(|c| c.changed_files.iter().any(|cf| cf == f || cf.ends_with(f)))
            .count();
        if commit_count >= 2
            && let Some((sym, sym_name, sym_file)) =
                find_preexisting_callable_in_file(f, state, baseline)
        {
            regressions.push(QualityRegression {
                kind: "short-horizon-churn".to_string(),
                sym_name,
                file_path: sym_file,
                line: sym.line,
                was_val: 0,
                now_val: commit_count as u32,
                is_gating: true,
                is_new_symbol: false,
            });
        }
    }
}

/// Evaluate quality delta for a workspace root.
pub fn evaluate_quality_delta(root: &str, state: &McpState) -> io::Result<QualityDeltaReport> {
    let at_stamp = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
    let head_sha = at_stamp
        .strip_suffix("+dirty")
        .unwrap_or(if at_stamp.is_empty() {
            "HEAD"
        } else {
            &at_stamp
        });
    let changed_files = crate::cli::safety::get_working_tree_changed_files(root);
    let mut working = compute_snapshot(&state.ingest, &state.graph, &state.interner, head_sha);
    working.dirty = !changed_files.is_empty();

    let baseline_path = Path::new(root).join(DEFAULT_BASELINE_FILE);
    let (baseline, source_name, head_basis) = if baseline_path.exists() {
        let snap = read_baseline_file(&baseline_path)?;
        (snap, "sidecar", None)
    } else if changed_files.is_empty() {
        (working.clone(), "git-HEAD", Some("identity"))
    } else {
        let snap = load_head_baseline_incremental(root, state, &working, &changed_files, head_sha);
        (snap, "git-HEAD", None)
    };

    let acks_path = Path::new(root).join(DEFAULT_ACKS_FILE);
    let acks = if acks_path.exists() {
        std::fs::read_to_string(&acks_path)
            .ok()
            .map(|c| parse_acks_content(&c))
    } else {
        None
    };

    let mut report =
        compute_quality_delta_with_acks(&working, &baseline, source_name, acks.as_ref());
    report.head_basis = head_basis;
    if !at_stamp.is_empty() {
        report.at_stamp = at_stamp;
    }
    if !changed_files.is_empty() {
        check_churn_regressions(root, state, &baseline, &mut report.regressions);
        if let Some(ledger) = &acks {
            let _ = apply_quality_acks(&mut report.regressions, ledger);
        }
    }
    report.gating_count = report.regressions.iter().filter(|r| r.is_gating).count();
    report.minor_count = report.regressions.len().saturating_sub(report.gating_count);
    report.preexisting_worse_count = report
        .regressions
        .iter()
        .filter(|r| !r.is_new_symbol)
        .count();

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::ToXmlString;

    #[test]
    fn test_quality_delta_complexity_regression() {
        let mut baseline = QualitySnapshot::default();
        baseline
            .ccx_by_sym
            .insert("src/math.rs::calc".to_string(), 10);

        let mut working = QualitySnapshot::default();
        // Now ccx grew from 10 to 20 (> 15 -> gating regression!)
        working
            .ccx_by_sym
            .insert("src/math.rs::calc".to_string(), 20);

        let report = compute_quality_delta(&working, &baseline, "test-baseline");
        assert_eq!(report.regressions.len(), 1);
        assert_eq!(report.gating_count, 1);
        assert!(report.has_gating_failures());

        let reg = &report.regressions[0];
        assert_eq!(reg.kind, "complexity");
        assert_eq!(reg.sym_name, "calc");
        assert_eq!(reg.was_val, 10);
        assert_eq!(reg.now_val, 20);
        assert!(reg.is_gating);
        assert!(!reg.is_new_symbol);

        let xml = report.to_xml_string();
        assert!(xml.contains("gating=\"1\""));
        assert!(xml.contains("<r kind=\"complexity\" sym=\"calc\""));
    }
    #[test]
    fn test_quality_delta_nesting_and_params_regressions() {
        let mut baseline = QualitySnapshot::default();
        baseline
            .ccx_by_sym
            .insert("src/math.rs::deep".to_string(), 5);
        baseline
            .nest_by_sym
            .insert("src/math.rs::deep".to_string(), 2);
        baseline
            .params_by_sym
            .insert("src/math.rs::deep".to_string(), 2);

        let mut working = QualitySnapshot::default();
        working
            .ccx_by_sym
            .insert("src/math.rs::deep".to_string(), 5);
        working
            .nest_by_sym
            .insert("src/math.rs::deep".to_string(), 7); // now > was and > 6 -> gating
        working
            .params_by_sym
            .insert("src/math.rs::deep".to_string(), 6); // now > was and > 5 -> gating

        let report = compute_quality_delta(&working, &baseline, "test-baseline");
        assert_eq!(report.regressions.len(), 2);
        assert_eq!(report.gating_count, 2);
        assert!(report.has_gating_failures());

        let kinds: Vec<&str> = report.regressions.iter().map(|r| r.kind.as_str()).collect();
        assert!(kinds.contains(&"nesting"));
        assert!(kinds.contains(&"params"));
    }

    #[test]
    fn test_quality_delta_acks_suppression() {
        let mut baseline = QualitySnapshot::default();
        baseline
            .ccx_by_sym
            .insert("src/math.rs::deep".to_string(), 5);
        baseline
            .nest_by_sym
            .insert("src/math.rs::deep".to_string(), 2);

        let mut working = QualitySnapshot::default();
        working
            .ccx_by_sym
            .insert("src/math.rs::deep".to_string(), 5);
        working
            .nest_by_sym
            .insert("src/math.rs::deep".to_string(), 7);

        let acks_content =
            "ack nesting 70a00a1f5e885d55 10 p=src/math.rs:1 deliberate deep nesting\n";
        let acks = parse_acks_content(acks_content);

        let report =
            compute_quality_delta_with_acks(&working, &baseline, "test-baseline", Some(&acks));
        assert_eq!(report.regressions.len(), 1);
        assert_eq!(report.gating_count, 0); // suppressed by ack!
        assert!(!report.has_gating_failures());
    }
    #[test]
    fn test_symbol_matches_and_changed() {
        use crate::domain::{FileInfo, IngestResult, Interner, Lang, SymKind, Symbol};

        let mut baseline = QualitySnapshot::default();
        baseline
            .ccx_by_sym
            .insert("src/math.rs::calc".to_string(), 10);
        baseline
            .loc_by_sym
            .insert("src/math.rs::calc".to_string(), 20);

        let interner = Interner::new();
        let sym_name = interner.intern("calc");
        let path = interner.intern("src/math.rs");
        let mut sym = Symbol::new(
            0,
            sym_name,
            interner.intern("src/math.rs::calc"),
            SymKind::Function,
            Lang::Rust,
            0,
            10,
        );
        sym.loc = 30; // changed from 20!
        sym.ccx = 10;
        let ingest = IngestResult {
            files: vec![FileInfo {
                id: 0,
                path,
                lang: Lang::Rust,
                byte_size: 100,
                line_count: 100,
            }],
            symbols: vec![sym],
            references: Vec::new(),
        };
        let changed =
            symbol_matches_and_changed(&sym, "src/math.rs", &ingest, &interner, &baseline);
        assert!(changed.is_some());
    }
}
