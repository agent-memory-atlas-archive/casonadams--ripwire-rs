use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::Path;

/// Generate Mermaid flowchart diagram from the repository call graph.
pub fn generate_mermaid_diagram<W: Write>(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    writeln!(out, "flowchart LR")?;

    let mut dir_by_node = Vec::with_capacity(ingest.symbols.len());
    let mut dirs = BTreeSet::new();

    for sym in &ingest.symbols {
        let p = interner.resolve(&ingest.files[sym.file_id as usize].path);
        let dir = Path::new(p)
            .parent()
            .and_then(|d| d.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(".");
        dir_by_node.push(dir.to_string());
        dirs.insert(dir.to_string());
    }

    let mut inter_module_calls: BTreeMap<(String, String), usize> = BTreeMap::new();

    for (node, from_dir) in dir_by_node.iter().enumerate() {
        for (callee, _) in graph.callees(node as u32) {
            if let Some(to_dir) = dir_by_node.get(callee as usize)
                && from_dir != to_dir
            {
                *inter_module_calls
                    .entry((from_dir.clone(), to_dir.clone()))
                    .or_insert(0) += 1;
            }
        }
    }

    for ((from, to), count) in inter_module_calls {
        let safe_from = from.replace('/', "_").replace('.', "root");
        let safe_to = to.replace('/', "_").replace('.', "root");
        writeln!(
            out,
            "  {safe_from}[\"{from}\"] -->|{count}| {safe_to}[\"{to}\"]"
        )?;
    }
    Ok(())
}
