use crate::domain::{IngestResult, Interner, NodeId};
use crate::gate::affected::is_test_path;
use crate::graph::Graph;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::Path;

/// A single cross-module edge lacking test coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeamEdge {
    pub caller: String,
    pub caller_path: String,
    pub callee: String,
    pub callee_path: String,
}

/// A cluster of untested call edges between two distinct directories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeamGroup {
    pub from_dir: String,
    pub to_dir: String,
    pub edges: Vec<SeamEdge>,
}

/// Comprehensive report of cross-module integration seams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeamsReport {
    pub modules_count: usize,
    pub bridges_count: usize,
    pub untested_count: usize,
    pub test_files_count: usize,
    pub seam_pairs_count: usize,
    pub groups: Vec<SeamGroup>,
    pub pr_iters: u32,
    pub gauges: crate::serialize::GraphGauges,
}

fn compute_directory(path: &str) -> String {
    Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(".")
        .to_string()
}

fn check_cross_module_edge(
    sym: &crate::domain::Symbol,
    target_id: NodeId,
    ingest: &IngestResult,
    interner: &Interner,
    tested_mask: &[bool],
) -> Option<(String, String, bool, Option<SeamEdge>)> {
    let from_file = ingest.sym_file_path(sym, interner);
    if is_test_path(from_file) {
        return None;
    }
    let from_dir = compute_directory(from_file);

    let target_sym = ingest.sym(target_id)?;
    let to_file = ingest.sym_file_path(target_sym, interner);
    if is_test_path(to_file) {
        return None;
    }
    let to_dir = compute_directory(to_file);
    if from_dir == to_dir {
        return None;
    }

    let sym_tested = tested_mask.get(sym.id as usize).copied().unwrap_or(false);
    let tgt_tested = tested_mask
        .get(target_id as usize)
        .copied()
        .unwrap_or(false);
    let is_untested = !sym_tested || !tgt_tested;
    let seam_edge = if is_untested {
        Some(SeamEdge {
            caller: interner[sym.name].to_string(),
            caller_path: format!("{from_file}:{}", sym.line),
            callee: interner[target_sym.name].to_string(),
            callee_path: format!("{to_file}:{}", target_sym.line),
        })
    } else {
        None
    };

    Some((from_dir, to_dir, true, seam_edge))
}

fn collect_module_and_test_files(
    ingest: &IngestResult,
    interner: &Interner,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut all_modules = BTreeSet::new();
    let mut test_files = BTreeSet::new();

    for f in &ingest.files {
        let p = interner.resolve(&f.path);
        all_modules.insert(compute_directory(p));
        if is_test_path(p) {
            test_files.insert(p.to_string());
        }
    }
    (all_modules, test_files)
}

fn process_sym_seams(
    sym: &crate::domain::Symbol,
    graph: &Graph,
    ingest: &IngestResult,
    interner: &Interner,
    tested_mask: &[bool],
    bridges_count: &mut usize,
    groups_map: &mut BTreeMap<(String, String), Vec<SeamEdge>>,
) {
    for (target_id, _) in graph.callees(sym.id) {
        if let Some((from_dir, to_dir, is_bridge, maybe_edge)) =
            check_cross_module_edge(sym, target_id, ingest, interner, tested_mask)
        {
            if is_bridge {
                *bridges_count += 1;
            }
            if let Some(edge) = maybe_edge {
                groups_map.entry((from_dir, to_dir)).or_default().push(edge);
            }
        }
    }
}

/// Detect untested cross-directory call edges across the workspace.
#[must_use]
pub fn find_integration_seams(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> SeamsReport {
    use crate::domain::CorpusView;
    let (all_modules, test_files) = collect_module_and_test_files(ingest, interner);
    let tested_mask = (ingest, interner).compute_test_reachability(graph);
    let mut bridges_count = 0usize;
    let mut groups_map: BTreeMap<(String, String), Vec<SeamEdge>> = BTreeMap::new();

    for sym in &ingest.symbols {
        process_sym_seams(
            sym,
            graph,
            ingest,
            interner,
            &tested_mask,
            &mut bridges_count,
            &mut groups_map,
        );
    }
    let mut groups = Vec::new();
    let mut untested_count = 0usize;

    for ((from_dir, to_dir), edges) in groups_map {
        untested_count += edges.len();
        groups.push(SeamGroup {
            from_dir,
            to_dir,
            edges,
        });
    }
    groups.sort_by(|a, b| {
        b.edges
            .len()
            .cmp(&a.edges.len())
            .then_with(|| a.from_dir.cmp(&b.from_dir))
            .then_with(|| a.to_dir.cmp(&b.to_dir))
    });
    SeamsReport {
        modules_count: all_modules.len(),
        bridges_count,
        untested_count,
        test_files_count: test_files.len(),
        seam_pairs_count: groups.len(),
        groups,
        pr_iters: graph
            .page_rank(&crate::rank::PageRankConfig::default())
            .iterations
            .max(1),
        gauges: graph.gauges(),
    }
}

impl crate::serialize::XmlReport for SeamsReport {
    fn schema_tag(&self) -> &'static str {
        "seams"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.seams/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some("ripwire seams schema=ripwire.seams/v1: cross-directory call edges NO test reaches.")
    }
    fn page_window(&self) -> Option<crate::serialize::PageWindow> {
        Some(crate::serialize::PageWindow::compute(
            self.seam_pairs_count,
            20,
            None,
            None,
        ))
    }
    fn pr_iters(&self) -> Option<u32> {
        Some(self.pr_iters)
    }
    fn graph_gauges(&self) -> Option<crate::serialize::GraphGauges> {
        Some(self.gauges)
    }
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        write!(
            w,
            " modules=\"{}\" bridges=\"{}\" untested=\"{}\" doc_links=\"0\" test_files=\"{}\" seam_pairs=\"{}\"",
            self.modules_count,
            self.bridges_count,
            self.untested_count,
            self.test_files_count,
            self.seam_pairs_count
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        for g in self.groups.iter().take(20) {
            let e_len = g.edges.len();
            let e_shown = e_len.min(5);
            let e_cap = usize::from(e_shown < e_len);
            writeln!(
                w,
                "  <seam from=\"{}\" to=\"{}\" untested=\"{e_len}\" shown=\"{e_shown}\" capped=\"{e_cap}\">",
                g.from_dir, g.to_dir
            )?;
            for e in g.edges.iter().take(e_shown) {
                writeln!(
                    w,
                    "    <edge caller=\"{}\" p=\"{}\" callee=\"{}\" cp=\"{}\"/>",
                    e.caller, e.caller_path, e.callee, e.callee_path
                )?;
            }
            writeln!(w, "  </seam>")?;
        }
        Ok(())
    }
}

/// Serialize seams report to canonical XML format.
pub fn serialize_seams_xml<W: Write>(root: &str, rep: &SeamsReport, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    rep.write_xml_report(root, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_directory() {
        assert_eq!(compute_directory("src/gate/seams.rs"), "src/gate");
        assert_eq!(compute_directory("lib.rs"), ".");
    }
}
