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
#[derive(Debug, Clone, Default)]
pub struct QualityAcks {
    pub acks: HashMap<(String, String), u32>,
}

impl QualityAcks {
    pub fn is_acknowledged(&self, kind: &str, key: &str, now_val: u32) -> bool {
        if let Some(&ack_val) = self.acks.get(&(kind.to_string(), key.to_string())) {
            return ack_val >= now_val || ack_val == 0;
        }
        let sym = key.split("::").last().unwrap_or(key);
        if let Some(&ack_val) = self.acks.get(&(kind.to_string(), sym.to_string())) {
            return ack_val >= now_val || ack_val == 0;
        }
        false
    }
}

pub fn parse_acks_content(content: &str) -> QualityAcks {
    let mut acks = HashMap::new();
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
            acks.entry((kind.clone(), key))
                .and_modify(|v: &mut u32| *v = (*v).max(val))
                .or_insert(val);
            for p in &parts[4..] {
                if let Some(path_part) = p.strip_prefix("p=") {
                    let sym_or_path = path_part.split(':').next().unwrap_or(path_part);
                    acks.entry((kind.clone(), sym_or_path.to_string()))
                        .and_modify(|v: &mut u32| *v = (*v).max(val))
                        .or_insert(val);
                    acks.entry((kind.clone(), path_part.to_string()))
                        .and_modify(|v: &mut u32| *v = (*v).max(val))
                        .or_insert(val);
                }
            }
        }
    }
    QualityAcks { acks }
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
    pub regressions: Vec<QualityRegression>,
    pub gating_count: usize,
    pub preexisting_worse_count: usize,
    pub new_symbol_count: usize,
    pub minor_count: usize,
    pub api_new_surface: usize,
}

impl crate::serialize::ToXmlString for QualityDeltaReport {
    fn write_xml<W: Write>(&self, out: &mut W) -> io::Result<()> {
        writeln!(
            out,
            "<quality-delta schema=\"ripwire.quality-delta/v1\" baseline=\"{}\" regressions=\"{}\" minor=\"{}\" acked=\"0\" stale=\"0\" preexisting-worse=\"{}\" new-symbol=\"{}\" gating=\"{}\" register-macro-excluded=\"0\" api-new-surface=\"{}\" at=\"{}\">",
            self.baseline_source,
            self.regressions.len(),
            self.minor_count,
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
            let gating_val = if r.is_gating { 1 } else { 0 };
            writeln!(
                out,
                "  <r kind=\"{}\" sym=\"{}\" p=\"{}:{}\" was=\"{}\" now=\"{}\" gating=\"{}\"{}/>",
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

fn apply_quality_acks(regressions: &mut [QualityRegression], ledger: &QualityAcks) {
    for r in regressions {
        let file = r.file_path.split(':').next().unwrap_or(&r.file_path);
        if ledger.is_acknowledged(&r.kind, &r.sym_name, r.now_val)
            || ledger.is_acknowledged(&r.kind, &r.file_path, r.now_val)
            || ledger.is_acknowledged(&r.kind, file, r.now_val)
        {
            r.is_gating = false;
        }
    }
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

    if let Some(ledger) = acks {
        apply_quality_acks(&mut regressions, ledger);
    }
    let (gating_count, preexisting_worse_count, new_symbol_count) =
        count_delta_categories(&regressions);
    let api_new_surface = working.public_api.difference(&baseline.public_api).count();
    let short_sha = if working.head_sha.len() >= 8 {
        &working.head_sha[..8]
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
        regressions,
        gating_count,
        preexisting_worse_count,
        new_symbol_count,
        minor_count,
        api_new_surface,
    }
}

fn load_head_baseline(root: &str, state: &McpState, head_sha: &str) -> QualitySnapshot {
    let mut head_files = Vec::new();
    let root_path = Path::new(root);

    for f in &state.ingest.files {
        let rel_path = state.interner.resolve(&f.path);
        if let Some(bytes) = crate::gate::contract::read_head_blob_bytes(root_path, rel_path) {
            head_files.push((rel_path.to_string(), f.lang, bytes));
        }
    }

    if head_files.is_empty() {
        return compute_snapshot(&state.ingest, &state.graph, &state.interner, head_sha);
    }

    let head_interner = crate::domain::Interner::new();
    let registry = match crate::ingest::LanguageRegistry::new() {
        Ok(r) => r,
        Err(_) => return compute_snapshot(&state.ingest, &state.graph, &state.interner, head_sha),
    };

    let head_file_slices: Vec<(&str, crate::domain::Lang, &[u8])> = head_files
        .iter()
        .map(|(p, l, b)| (p.as_str(), *l, b.as_slice()))
        .collect();

    let head_ingest =
        crate::ingest::ingest_in_memory_files(&head_file_slices, &registry, &head_interner);
    let head_graph = crate::graph::Graph::build(&head_ingest, &head_interner);
    compute_snapshot(&head_ingest, &head_graph, &head_interner, head_sha)
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
    let baseline_path = Path::new(root).join(DEFAULT_BASELINE_FILE);

    let (baseline, source_name) = if baseline_path.exists() {
        let snap = read_baseline_file(&baseline_path)?;
        (snap, "sidecar")
    } else {
        let head_sha = state
            .commits
            .as_ref()
            .and_then(|c| c.first())
            .map(|c| c.id.as_str())
            .unwrap_or("HEAD");
        let snap = load_head_baseline(root, state, head_sha);
        (snap, "git-HEAD")
    };
    let working_sha = state
        .commits
        .as_ref()
        .and_then(|c| c.first())
        .map(|c| c.id.as_str())
        .unwrap_or("HEAD");
    let working = compute_snapshot(&state.ingest, &state.graph, &state.interner, working_sha);

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
    check_churn_regressions(root, state, &baseline, &mut report.regressions);
    if let Some(ledger) = &acks {
        for r in &mut report.regressions {
            let file = r.file_path.split(':').next().unwrap_or(&r.file_path);
            if ledger.is_acknowledged(&r.kind, &r.sym_name, r.now_val)
                || ledger.is_acknowledged(&r.kind, &r.file_path, r.now_val)
                || ledger.is_acknowledged(&r.kind, file, r.now_val)
            {
                r.is_gating = false;
            }
        }
    }
    report.gating_count = report.regressions.iter().filter(|r| r.is_gating).count();
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
