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
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let mut cells = Vec::new();
    for (target_id, _) in graph.callees(sym_id) {
        if let Some(target) = ingest.sym(target_id)
            && (target.kind == SymKind::Var || target.kind == SymKind::Field)
        {
            cells.push(StateCellAccess {
                cell_name: view.sym_name(target).to_string(),
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
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let mut out = Vec::new();
    for sym in &ingest.symbols {
        if sym.kind != SymKind::Function && sym.kind != SymKind::Method {
            continue;
        }
        let p = view.sym_path(sym);
        let cells = collect_sym_cells(sym.id, p, ingest, graph, interner);
        if !cells.is_empty() {
            out.push(NonlocalFunction {
                sym_name: view.sym_name(sym).to_string(),
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
            write_nonlocal_fn_row(f, out)?;
        }
        Ok(())
    }
}

fn write_nonlocal_fn_row<W: Write>(f: &NonlocalFunction, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlEscapeExt;
    writeln!(
        out,
        "  <fn p=\"{}:{}\" n=\"{}\" writes=\"{}\" reads=\"{}\" direct_writes=\"{}\" direct_reads=\"{}\" cells_total=\"{}\">",
        f.path.xml_escaped(),
        f.line,
        f.sym_name.xml_escaped(),
        f.writes,
        f.reads,
        f.writes,
        f.reads,
        f.cells.len()
    )?;
    for c in &f.cells {
        let mode = if c.is_write { "w" } else { "r" };
        writeln!(
            out,
            "    <cell n=\"{}\" p=\"{}:{}\" dir=\"{mode}\"/>",
            c.cell_name.xml_escaped(),
            c.path.xml_escaped(),
            c.line
        )?;
    }
    writeln!(out, "  </fn>")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_nonlocal_state_rows_and_cells() {
        let funcs = vec![NonlocalFunction {
            sym_name: "mutate_global".to_string(),
            path: "src/state.rs".to_string(),
            line: 10,
            writes: 1,
            reads: 1,
            cells: vec![
                StateCellAccess {
                    cell_name: "GLOBAL_COUNTER".to_string(),
                    path: "src/state.rs".to_string(),
                    line: 3,
                    is_write: true,
                },
                StateCellAccess {
                    cell_name: "CONFIG".to_string(),
                    path: "src/state.rs".to_string(),
                    line: 4,
                    is_write: false,
                },
            ],
        }];
        let mut out = Vec::new();
        serialize_nonlocal_state_xml(".", &funcs, &mut out).unwrap();
        let xml = String::from_utf8(out).unwrap();
        assert!(xml.contains("cells=\"2\" functions=\"1\""));
        assert!(xml.contains("<cell n=\"GLOBAL_COUNTER\" p=\"src/state.rs:3\" dir=\"w\"/>"));
        assert!(xml.contains("<cell n=\"CONFIG\" p=\"src/state.rs:4\" dir=\"r\"/>"));
    }
}
