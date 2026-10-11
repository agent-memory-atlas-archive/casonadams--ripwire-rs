use crate::gate::baseline::{
    compute_snapshot, read_baseline_file, QualitySnapshot, DEFAULT_BASELINE_FILE,
};
use crate::mcp::McpState;
use std::io::{self, Write};
use std::path::Path;

/// A single detected quality regression between working tree and baseline.
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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QualityDeltaReport {
    pub baseline_source: String,
    pub head_sha: String,
    pub regressions: Vec<QualityRegression>,
    pub gating_count: usize,
    pub preexisting_worse_count: usize,
    pub new_symbol_count: usize,
}

impl crate::serialize::ToXmlString for QualityDeltaReport {
    fn write_xml<W: Write>(&self, out: &mut W) -> io::Result<()> {
        writeln!(
            out,
            "<quality-delta baseline=\"{}\" head=\"{}\" regressions=\"{}\" gating=\"{}\" preexisting_worse=\"{}\" new_symbols=\"{}\">",
            self.baseline_source,
            self.head_sha,
            self.regressions.len(),
            self.gating_count,
            self.preexisting_worse_count,
            self.new_symbol_count
        )?;
        for r in &self.regressions {
            let origin_attr = if r.is_new_symbol {
                " origin=\"new-symbol\""
            } else {
                ""
            };
            let gating_attr = if r.is_gating { " gating=\"1\"" } else { "" };
            writeln!(
                out,
                "  <r kind=\"{}\" sym=\"{}\" p=\"{}:{}\" was=\"{}\" now=\"{}\"{}{}/>",
                r.kind,
                r.sym_name,
                r.file_path,
                r.line,
                r.was_val,
                r.now_val,
                origin_attr,
                gating_attr
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
    let mut regressions = Vec::new();

    for key in working.ccx_by_sym.keys() {
        let is_preexisting = baseline.ccx_by_sym.contains_key(key);
        check_symbol_numeric_regressions(key, working, baseline, is_preexisting, &mut regressions);
    }

    check_structural_regressions(working, baseline, &mut regressions);

    let mut gating_count = 0usize;
    let mut preexisting_worse_count = 0usize;
    let mut new_symbol_count = 0usize;

    for r in &regressions {
        if r.is_gating {
            gating_count += 1;
        }
        if r.is_new_symbol {
            new_symbol_count += 1;
        } else {
            preexisting_worse_count += 1;
        }
    }

    QualityDeltaReport {
        baseline_source: baseline_source.to_string(),
        head_sha: working.head_sha.clone(),
        regressions,
        gating_count,
        preexisting_worse_count,
        new_symbol_count,
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
        let snap = compute_snapshot(&state.ingest, &state.graph, &state.interner, head_sha);
        (snap, "git-HEAD")
    };

    let working_sha = state
        .commits
        .as_ref()
        .and_then(|c| c.first())
        .map(|c| c.id.as_str())
        .unwrap_or("HEAD");
    let working = compute_snapshot(&state.ingest, &state.graph, &state.interner, working_sha);

    Ok(compute_quality_delta(&working, &baseline, source_name))
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
}
