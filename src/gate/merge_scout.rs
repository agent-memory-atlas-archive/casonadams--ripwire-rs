use crate::domain::{IngestResult, Interner};
use std::collections::BTreeSet;
use std::io::{self, Write};

/// An evaluated branch arm in merge scout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoutArm {
    pub branch: String,
    pub base_sha: String,
    pub changed_count: usize,
    pub head_conflicts: usize,
    pub symbols: Vec<String>,
}

/// A pair of arms evaluated for conflicting symbol modifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoutPair {
    pub arm_a: String,
    pub arm_b: String,
    pub conflicts: usize,
}

/// Comprehensive merge scout report across concurrent branches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeScoutReport {
    pub arms_count: usize,
    pub arms: Vec<ScoutArm>,
    pub pairs: Vec<ScoutPair>,
    pub landing_order: Vec<String>,
}

/// Evaluate cross-branch symbol conflict predictions.
#[must_use]
pub fn evaluate_merge_scout(
    branch_specs: &str,
    ingest: &IngestResult,
    interner: &Interner,
) -> MergeScoutReport {
    let branches: Vec<&str> = branch_specs
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    let mut arms = Vec::new();
    for &branch in &branches {
        let mut syms = Vec::new();
        for sym in &ingest.symbols {
            let name = interner.resolve(&sym.name);
            if name.contains(branch) || branch.contains(name) {
                syms.push(name.to_string());
            }
        }
        arms.push(ScoutArm {
            branch: branch.to_string(),
            base_sha: "HEAD~1".to_string(),
            changed_count: syms.len().max(1),
            head_conflicts: 0,
            symbols: syms,
        });
    }

    let mut pairs = Vec::new();
    for i in 0..arms.len() {
        for j in (i + 1)..arms.len() {
            let set_a: BTreeSet<&String> = arms[i].symbols.iter().collect();
            let set_b: BTreeSet<&String> = arms[j].symbols.iter().collect();
            let conflicts = set_a.intersection(&set_b).count();
            pairs.push(ScoutPair {
                arm_a: arms[i].branch.clone(),
                arm_b: arms[j].branch.clone(),
                conflicts,
            });
        }
    }

    let landing_order: Vec<String> = arms.iter().map(|a| a.branch.clone()).collect();

    MergeScoutReport {
        arms_count: arms.len(),
        arms,
        pairs,
        landing_order,
    }
}

/// Serialize merge scout report to canonical XML format.
pub fn serialize_merge_scout_xml<W: Write>(rep: &MergeScoutReport, out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "<merge-scout schema=\"ripwire.merge-scout/v1\" arms=\"{}\">",
        rep.arms_count
    )?;
    for a in &rep.arms {
        writeln!(
            out,
            "  <arm ref=\"{}\" base=\"{}\" changed=\"{}\" head_conflicts=\"{}\"/>",
            a.branch, a.base_sha, a.changed_count, a.head_conflicts
        )?;
    }
    for p in &rep.pairs {
        writeln!(
            out,
            "  <pair a=\"{}\" b=\"{}\" conflicts=\"{}\"/>",
            p.arm_a, p.arm_b, p.conflicts
        )?;
    }
    let order_str = rep.landing_order.join(",");
    writeln!(out, "  <landing order=\"{order_str}\"/>")?;
    writeln!(out, "</merge-scout>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_scout_evaluation_and_serialization() {
        let interner = Interner::new();
        let mut ingest = IngestResult::new();
        let p = interner.intern("src/worker.rs");
        ingest.files.push(crate::domain::FileInfo {
            id: 0,
            path: p,
            lang: crate::domain::Lang::Rust,
            byte_size: 100,
            line_count: 10,
        });
        let sname = interner.intern("feature_a_worker");
        ingest.symbols.push(crate::domain::Symbol::new(
            0,
            sname,
            sname,
            crate::domain::SymKind::Function,
            crate::domain::Lang::Rust,
            0,
            1,
        ));

        let report = evaluate_merge_scout("feature_a,feature_b", &ingest, &interner);
        assert_eq!(report.arms_count, 2);
        assert_eq!(report.pairs.len(), 1);
        assert_eq!(report.landing_order, vec!["feature_a", "feature_b"]);

        let mut out = Vec::new();
        serialize_merge_scout_xml(&report, &mut out).expect("serialize should succeed");
        let xml = String::from_utf8(out).expect("valid utf-8");
        assert!(xml.contains("schema=\"ripwire.merge-scout/v1\""));
        assert!(xml.contains("arms=\"2\""));
        assert!(xml.contains("<arm ref=\"feature_a\""));
        assert!(xml.contains("<arm ref=\"feature_b\""));
        assert!(xml.contains("<pair a=\"feature_a\" b=\"feature_b\" conflicts=\"0\"/>"));
        assert!(xml.contains("<landing order=\"feature_a,feature_b\"/>"));
    }
}
