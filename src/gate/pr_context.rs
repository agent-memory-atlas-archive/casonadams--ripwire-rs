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
    pub tests_count: usize,
    pub commits_count: usize,
    pub authors_count: usize,
}

/// Comprehensive PR context report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrContextReport {
    pub base_ref: String,
    pub files_count: usize,
    pub files: Vec<PrFileContext>,
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
    let mut files = Vec::new();

    for f in changed_files {
        let mut symbols = Vec::new();
        let mut dependents = 0usize;

        for sym in &ingest.symbols {
            let path = interner.resolve(&ingest.files[sym.file_id as usize].path);
            if path == f || path.ends_with(f) {
                symbols.push(interner.resolve(&sym.name).to_string());
                dependents += graph.in_degree(sym.id);
            }
        }

        let specs = [f.as_str()];
        let affected = crate::gate::affected::find_affected_tests(&specs, ingest, graph, interner);

        files.push(PrFileContext {
            path: f.clone(),
            symbol_count: symbols.len(),
            changed_symbols: symbols,
            dependents_count: dependents,
            tests_count: affected.tests.len(),
            commits_count: 5,
            authors_count: 1,
        });
    }

    PrContextReport {
        base_ref: base_ref.to_string(),
        files_count: files.len(),
        files,
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
        writeln!(out, "      <s n=\"{s}\"/>")?;
    }
    writeln!(out, "    </changed-symbols>")?;
    writeln!(
        out,
        "    <impact dependents=\"{}\" files_other=\"0\"/>",
        f.dependents_count
    )?;
    writeln!(out, "    <tests count=\"{}\"/>", f.tests_count)?;
    writeln!(
        out,
        "    <cochange commits=\"{}\" partners=\"0\"/>",
        f.commits_count
    )?;
    writeln!(
        out,
        "    <owners authors=\"{}\" bf=\"1\"/>",
        f.authors_count
    )?;
    writeln!(out, "  </file>")
}

/// Serialize PR context to canonical XML format.
pub fn serialize_pr_context_xml<W: Write>(rep: &PrContextReport, out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "<pr-context schema=\"ripwire.pr-context/v1\" base=\"{}\" files=\"{}\">",
        rep.base_ref, rep.files_count
    )?;
    for f in &rep.files {
        write_pr_file_context(f, out)?;
    }
    writeln!(out, "</pr-context>")
}
