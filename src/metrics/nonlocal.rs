use crate::domain::{IngestResult, Interner, SymKind};
use crate::graph::Graph;
use std::io::{self, Write};

/// An accessed nonlocal state cell (global or static variable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateCellAccess {
    pub cell_name: String,
    pub path: String,
    pub line: u32,
    pub is_write: bool,
}

/// Function with nonlocal state accesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonlocalFunction {
    pub sym_name: String,
    pub path: String,
    pub line: u32,
    pub writes: usize,
    pub reads: usize,
    pub cells: Vec<StateCellAccess>,
}

/// Find functions that read or write nonlocal mutable state.
#[must_use]
fn collect_sym_cells(
    sym_id: crate::domain::NodeId,
    file_path: &str,
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> Vec<StateCellAccess> {
    let mut cells = Vec::new();
    for (target_id, _) in graph.callees(sym_id) {
        if let Some(target) = ingest.symbols.get(target_id as usize)
            && (target.kind == SymKind::Var || target.kind == SymKind::Field)
        {
            cells.push(StateCellAccess {
                cell_name: interner.resolve(&target.name).to_string(),
                path: file_path.to_string(),
                line: target.line,
                is_write: false,
            });
        }
    }
    cells
}

pub fn find_nonlocal_state(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> Vec<NonlocalFunction> {
    let mut out = Vec::new();
    for sym in &ingest.symbols {
        if sym.kind != SymKind::Function && sym.kind != SymKind::Method {
            continue;
        }
        let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
        let cells = collect_sym_cells(sym.id, p, ingest, graph, interner);
        if !cells.is_empty() {
            out.push(NonlocalFunction {
                sym_name: interner.resolve(&sym.name).to_string(),
                path: p.to_string(),
                line: sym.line,
                writes: 0,
                reads: cells.len(),
                cells,
            });
        }
    }
    out
}

pub struct NonlocalStateReport<'a> {
    pub funcs: &'a [NonlocalFunction],
    pub gauges: crate::serialize::GraphGauges,
}

impl crate::serialize::XmlReport for NonlocalStateReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "nonlocal_state"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.nonlocal-state/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire nonlocal-state schema=ripwire.nonlocal-state/v1: per function, the non-local MUTABLE state it reaches.",
        )
    }
    fn graph_gauges(&self) -> Option<crate::serialize::GraphGauges> {
        Some(self.gauges)
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let count = self.funcs.len();
        let cells_count: usize = self.funcs.iter().map(|f| f.cells.len()).sum();
        write!(
            out,
            " cells=\"{cells_count}\" functions=\"{count}\" shown=\"{count}\" capped=\"0\" unanalyzed_langs=\"\" unanalyzed_files=\"0\""
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for f in self.funcs {
            let p_raw = format!("{}:{}", f.path, f.line);
            let p_esc = crate::serialize::escape::escape_xml_str(&p_raw);
            let n_esc = crate::serialize::escape::escape_xml_str(&f.sym_name);
            let total_c = f.cells.len();
            writeln!(
                out,
                "  <fn p=\"{p_esc}\" n=\"{n_esc}\" writes=\"{}\" reads=\"{}\" direct_writes=\"{}\" direct_reads=\"{}\" cells_total=\"{total_c}\">",
                f.writes, f.reads, f.writes, f.reads
            )?;
            for c in &f.cells {
                let mode = if c.is_write { "w" } else { "r" };
                let cell_n = crate::serialize::escape::escape_xml_str(&c.cell_name);
                let cell_p_raw = format!("{}:{}", c.path, c.line);
                let cell_p = crate::serialize::escape::escape_xml_str(&cell_p_raw);
                writeln!(
                    out,
                    "    <cell n=\"{cell_n}\" p=\"{cell_p}\" dir=\"{mode}\"/>",
                )?;
            }
            writeln!(out, "  </fn>")?;
        }
        Ok(())
    }
}

/// Serialize nonlocal state report to canonical XML format.
pub fn serialize_nonlocal_state_xml<W: Write>(
    root: &str,
    funcs: &[NonlocalFunction],
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    NonlocalStateReport {
        funcs,
        gauges: crate::serialize::GraphGauges::default(),
    }
    .write_xml_report(root, out)
}

pub fn serialize_nonlocal_state_with_gauge_xml<W: Write>(
    root: &str,
    funcs: &[NonlocalFunction],
    gauges: crate::serialize::GraphGauges,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    NonlocalStateReport { funcs, gauges }.write_xml_report(root, out)
}
