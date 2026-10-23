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
    pub history: Option<FileHistory>,
}

/// Git history of one changed file. Absent when the file has no commits, so no cochange or
/// owner claim is made for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHistory {
    pub commits: usize,
    pub partners: usize,
    pub authors: usize,
    pub bus_factor: usize,
}

/// Comprehensive PR context report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrContextReport {
    pub base_ref: Option<String>,
    pub at_stamp: String,
    pub files_count: usize,
    pub files: Vec<PrFileContext>,
    pub gauges: crate::serialize::GraphGauges,
    pub page: crate::serialize::PageRequest,
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

    use crate::domain::CorpusView;
    let view = (ingest, interner);
    for sym in &ingest.symbols {
        let path = view.sym_path(sym);
        if path == f || path.ends_with(f) {
            symbols.push(view.sym_name(sym).to_string());
            dependents += graph.in_degree(sym.id);
            for (caller_id, _) in graph.callers(sym.id) {
                if let Some(cs) = ingest.sym(caller_id)
                    && cs.file_id != sym.file_id
                {
                    caller_files.insert(cs.file_id);
                }
            }
        }
    }

    let specs = [f];
    let affected = crate::gate::affected::find_affected_tests(&specs, ingest, graph, interner);
    let mut file_commits = 0usize;
    let mut authors = std::collections::HashSet::new();
    for c in commits {
        if c.changed_files.iter().any(|cf| cf == f || cf.ends_with(f)) {
            file_commits += 1;
            authors.insert(c.author.as_str());
        }
    }
    let history = (file_commits > 0).then(|| FileHistory {
        commits: file_commits,
        partners: coupling.partners_of(f).len(),
        authors: authors.len(),
        bus_factor: authors.len().min(2),
    });

    PrFileContext {
        path: f.to_string(),
        symbol_count: symbols.len(),
        changed_symbols: symbols,
        dependents_count: dependents,
        files_other_count: caller_files.len(),
        tests_count: affected.tests.len(),
        history,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PrContextRequest<'a> {
    pub changed_files: &'a [String],
    pub base_ref: Option<&'a str>,
    pub at_stamp: &'a str,
    pub commits: &'a [crate::git::history::CommitInfo],
    pub page: crate::serialize::PageRequest,
}

#[must_use]
pub fn compute_pr_context_from_facts(
    req: &PrContextRequest<'_>,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> PrContextReport {
    let coupling = crate::git::cochange::StaticCouplingMap::build(ingest, interner, Some(graph));
    let files: Vec<PrFileContext> = req
        .changed_files
        .iter()
        .map(|f| build_single_pr_file_context(f, ingest, graph, interner, &coupling, req.commits))
        .collect();

    PrContextReport {
        base_ref: req.base_ref.map(str::to_string),
        at_stamp: req.at_stamp.to_string(),
        files_count: files.len(),
        files,
        gauges: graph.gauges(),
        page: req.page,
    }
}

/// Serialize PR context to canonical XML format.
fn write_pr_file_context(f: &PrFileContext, out: &mut dyn Write) -> io::Result<()> {
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
    if let Some(h) = &f.history {
        writeln!(
            out,
            "    <cochange commits=\"{}\" partners=\"{}\"/>",
            h.commits, h.partners
        )?;
        writeln!(
            out,
            "    <owners authors=\"{}\" bf=\"{}\"/>",
            h.authors, h.bus_factor
        )?;
    }
    writeln!(out, "  </file>")
}

impl PrContextReport {
    fn files_paged(&self) -> crate::serialize::Paged<'_, PrFileContext> {
        crate::serialize::Paged::new(&self.files, self.page, |f| {
            crate::serialize::row_bytes(|w| write_pr_file_context(f, w))
        })
    }
}

impl crate::serialize::XmlReport for PrContextReport {
    crate::xml_report_meta!(
        "pr-context",
        "ripwire-rs.pr-context/v1",
        root: true,
        comment: "ripwire-rs pr-context schema=ripwire-rs.pr-context/v1: review bundle per changed file vs base=."
    );
    fn page_window(&self) -> Option<crate::serialize::PageWindow> {
        Some(self.files_paged().window)
    }
    fn graph_gauges(&self) -> Option<crate::serialize::GraphGauges> {
        Some(self.gauges)
    }
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let est_tokens = 180 + self.files.len() * 140;
        let window = self.files_paged().window;
        let budget = self
            .page
            .budget
            .unwrap_or(crate::serialize::DEFAULT_PACK_BUDGET_BYTES);
        let budget_tokens = budget / crate::serialize::BYTES_PER_TOKEN;
        let trim_level = usize::from(window.capped);
        let truncated = if window.capped { "files" } else { "none" };
        let budget_default = usize::from(self.page.budget.is_none());
        let base = self.base_ref.as_deref().unwrap_or("working-tree");
        let direction = if self.base_ref.is_some() {
            "worktree-since-merge-base"
        } else {
            "worktree-since-head"
        };
        let at_attr = if self.at_stamp.is_empty() {
            String::new()
        } else {
            format!(" at=\"{}\"", crate::serialize::XmlEscaped(&self.at_stamp))
        };
        write!(
            w,
            " base=\"{}\" direction=\"{direction}\" files=\"{}\" budget_tokens=\"{budget_tokens}\" est_tokens=\"{est_tokens}\" trim_level=\"{trim_level}\" truncated=\"{truncated}\" budget_default=\"{budget_default}\"{at_attr}",
            crate::serialize::XmlEscaped(base),
            self.files_count
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        for f in self.files_paged().rows() {
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
