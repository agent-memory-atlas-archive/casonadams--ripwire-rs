use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use std::io::{self, Write};

/// PR Review context for a single modified file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrFileContext {
    pub path: String,
    pub symbol_count: usize,
    pub changed_symbols: Vec<String>,
    pub dependents_count: usize,
    pub files_other_count: usize,
    pub tests_count: usize,
    pub commits_count: usize,
    pub partners_count: usize,
    pub authors_count: usize,
    pub bus_factor: usize,
}

/// Comprehensive PR context report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrContextReport {
    pub base_ref: String,
    pub files_count: usize,
    pub files: Vec<PrFileContext>,
    pub gauges: crate::serialize::GraphGauges,
}

fn build_single_pr_file_context(
    f: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    coupling: &crate::git::cochange::StaticCouplingMap,
    commits: &[crate::git::history::CommitInfo],
) -> PrFileContext {
    let mut symbols = Vec::new();
    let mut dependents = 0usize;
    let mut caller_files = std::collections::HashSet::new();

    for sym in &ingest.symbols {
        let path = interner.resolve(&ingest.files[sym.file_id as usize].path);
        if path == f || path.ends_with(f) {
            symbols.push(interner.resolve(&sym.name).to_string());
            dependents += graph.in_degree(sym.id);
            for (caller_id, _) in graph.callers(sym.id) {
                if let Some(cs) = ingest.symbols.get(caller_id as usize)
                    && cs.file_id != sym.file_id
                {
                    caller_files.insert(cs.file_id);
                }
            }
        }
    }

    let specs = [f];
    let affected = crate::gate::affected::find_affected_tests(&specs, ingest, graph, interner);
    let partners_count = coupling.partners_of(f).len();
    let mut file_commits = 0usize;
    let mut authors = std::collections::HashSet::new();
    for c in commits {
        if c.changed_files.iter().any(|cf| cf == f || cf.ends_with(f)) {
            file_commits += 1;
            authors.insert(c.author.as_str());
        }
    }
    let commits_count = file_commits.max(1);
    let authors_count = authors.len().max(1);
    let bus_factor = authors_count.min(2);

    PrFileContext {
        path: f.to_string(),
        symbol_count: symbols.len(),
        changed_symbols: symbols,
        dependents_count: dependents,
        files_other_count: caller_files.len(),
        tests_count: affected.tests.len(),
        commits_count,
        partners_count,
        authors_count,
        bus_factor,
    }
}

/// Compute PR context for changed files against baseline.
#[must_use]
pub fn compute_pr_context(
    changed_files: &[String],
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    base_ref: &str,
) -> PrContextReport {
    compute_pr_context_in_root(".", changed_files, ingest, graph, interner, base_ref)
}

#[must_use]
pub fn compute_pr_context_in_root(
    root: &str,
    changed_files: &[String],
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    base_ref: &str,
) -> PrContextReport {
    let coupling = crate::git::cochange::StaticCouplingMap::build(ingest, interner, Some(graph));
    let commits = crate::git::history::walk_repository_history(std::path::Path::new(root))
        .unwrap_or_default();
    let files: Vec<PrFileContext> = changed_files
        .iter()
        .map(|f| build_single_pr_file_context(f, ingest, graph, interner, &coupling, &commits))
        .collect();

    PrContextReport {
        base_ref: base_ref.to_string(),
        files_count: files.len(),
        files,
        gauges: graph.gauges(),
    }
}

/// Serialize PR context to canonical XML format.
fn write_pr_file_context<W: Write>(f: &PrFileContext, out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "  <file p=\"{}\" symbols=\"{}\">",
        f.path, f.symbol_count
    )?;
    writeln!(
        out,
        "    <changed-symbols count=\"{}\">",
        f.changed_symbols.len()
    )?;
    for s in &f.changed_symbols {
        writeln!(out, "      <s n=\"{}\"/>", crate::serialize::XmlEscaped(s))?;
    }
    writeln!(out, "    </changed-symbols>")?;
    writeln!(
        out,
        "    <impact dependents=\"{}\" files_other=\"{}\"/>",
        f.dependents_count, f.files_other_count
    )?;
    writeln!(out, "    <tests count=\"{}\"/>", f.tests_count)?;
    writeln!(
        out,
        "    <cochange commits=\"{}\" partners=\"{}\"/>",
        f.commits_count, f.partners_count
    )?;
    writeln!(
        out,
        "    <owners authors=\"{}\" bf=\"{}\"/>",
        f.authors_count, f.bus_factor
    )?;
    writeln!(out, "  </file>")
}

impl crate::serialize::XmlReport for PrContextReport {
    fn schema_tag(&self) -> &'static str {
        "pr-context"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.pr-context/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire pr-context schema=ripwire.pr-context/v1: review bundle per changed file vs base=.",
        )
    }
    fn graph_gauges(&self) -> Option<crate::serialize::GraphGauges> {
        Some(self.gauges)
    }
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let est_tokens = 180 + self.files.len() * 140;
        write!(
            w,
            " base=\"{}\" direction=\"worktree-since-head\" files=\"{}\" skipped_mode_only=\"0\" budget_tokens=\"8000\" est_tokens=\"{est_tokens}\" trim_level=\"0\" truncated=\"none\" budget_default=\"1\" at=\"HEAD\"",
            self.base_ref, self.files_count
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        for f in &self.files {
            write_pr_file_context(f, w)?;
        }
        Ok(())
    }
}

/// Serialize PR context to canonical XML format.
pub fn serialize_pr_context_xml<W: Write>(rep: &PrContextReport, out: &mut W) -> io::Result<()> {
    use crate::serialize::ToXmlString;
    rep.write_xml(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_pr_context_xml() {
        let report = PrContextReport {
            base_ref: "main".to_string(),
            files_count: 1,
            files: vec![PrFileContext {
                path: "src/lib.rs".to_string(),
                symbol_count: 5,
                changed_symbols: vec!["run_cli".to_string(), "init".to_string()],
                dependents_count: 3,
                files_other_count: 0,
                tests_count: 2,
                commits_count: 4,
                partners_count: 0,
                authors_count: 1,
                bus_factor: 1,
            }],
            gauges: crate::serialize::GraphGauges::default(),
        };
        let mut out = Vec::new();
        serialize_pr_context_xml(&report, &mut out).expect("serialize should succeed");
        let xml = String::from_utf8(out).expect("valid utf-8");
        assert!(xml.contains("schema=\"ripwire.pr-context/v1\""));
        assert!(xml.contains("base=\"main\""));
        assert!(xml.contains("<file p=\"src/lib.rs\" symbols=\"5\">"));
        assert!(xml.contains("<s n=\"run_cli\"/>"));
        assert!(xml.contains("<s n=\"init\"/>"));
        assert!(xml.contains("<impact dependents=\"3\" files_other=\"0\"/>"));
        assert!(xml.contains("<tests count=\"2\"/>"));
        assert!(xml.contains("<cochange commits=\"4\" partners=\"0\"/>"));
        assert!(xml.contains("<owners authors=\"1\" bf=\"1\"/>"));
    }

    #[test]
    fn test_compute_pr_context_report() {
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
        let sname = interner.intern("run_job");
        ingest.symbols.push(crate::domain::Symbol::new(
            0,
            sname,
            sname,
            crate::domain::SymKind::Function,
            crate::domain::Lang::Rust,
            0,
            5,
        ));
        let graph = Graph::build(&ingest, &interner);
        let report = compute_pr_context(
            &["src/worker.rs".to_string()],
            &ingest,
            &graph,
            &interner,
            "HEAD~1",
        );
        assert_eq!(report.base_ref, "HEAD~1");
        assert_eq!(report.files_count, 1);
        assert_eq!(report.files[0].path, "src/worker.rs");
        assert_eq!(report.files[0].changed_symbols, vec!["run_job"]);
    }
}
