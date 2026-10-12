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
        let n = interner.resolve(&sym.name);
        let writes = 0;
        let mut reads = 0;
        let mut cells = Vec::new();

        for (target_id, _) in graph.callees(sym.id) {
            if let Some(target) = ingest.symbols.get(target_id as usize)
                && (target.kind == SymKind::Var || target.kind == SymKind::Field)
            {
                reads += 1;
                cells.push(StateCellAccess {
                    cell_name: interner.resolve(&target.name).to_string(),
                    path: p.to_string(),
                    line: target.line,
                    is_write: false,
                });
            }
        }

        if reads > 0 || writes > 0 {
            out.push(NonlocalFunction {
                sym_name: n.to_string(),
                path: p.to_string(),
                line: sym.line,
                writes,
                reads,
                cells,
            });
        }
    }
    out
}

/// Serialize nonlocal state report to canonical XML format.
pub fn serialize_nonlocal_state_xml<W: Write>(
    root: &str,
    funcs: &[NonlocalFunction],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<nonlocal_state schema=\"ripwire.nonlocal-state/v1\" root=\"{root}\" functions=\"{}\">",
        funcs.len()
    )?;
    for f in funcs {
        writeln!(
            out,
            "  <fn p=\"{}:{}\" n=\"{}\" writes=\"{}\" reads=\"{}\">",
            f.path, f.line, f.sym_name, f.writes, f.reads
        )?;
        for c in &f.cells {
            let mode = if c.is_write { "w" } else { "r" };
            writeln!(
                out,
                "    <cell n=\"{}\" p=\"{}:{}\" dir=\"{mode}\"/>",
                c.cell_name, c.path, c.line
            )?;
        }
        writeln!(out, "  </fn>")?;
    }
    writeln!(out, "</nonlocal_state>")
}
