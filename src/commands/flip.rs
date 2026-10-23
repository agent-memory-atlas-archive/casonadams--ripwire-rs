//! `--flip GATE`: the reach of toggling one feature flag from the `--flags` table.
use super::plugin::report_command;
use crate::commands::context::{ExecutionContext, Paginated};
use crate::domain::{IngestResult, Interner, NodeId};
use crate::gate::affected::AffectedTest;
use crate::graph::Graph;
use crate::navigate::{ImpactNode, inspect::resolve_symbol_chain_at, transitive_impact_multi};
use crate::query::FileSources;
use crate::query::doc_tools;
use crate::serialize::{PageRequest, Paged, XmlEscapeExt, XmlWriter, text_bytes};
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;

report_command! {
    name: "flip",
    args: FlipArgs,
    static: FLIP,
    run: report(render),
    /// Reach of toggling one feature flag: its reading symbols, their callers and affected tests
    #[arg(long = "flip", value_name = "GATE")]
    flip: Option<String>,
}

pub fn render<W: Write>(
    ctx: &ExecutionContext<'_>,
    args: &FlipArgs,
    out: &mut XmlWriter<'_, W>,
) -> io::Result<()> {
    let gate = args.flip.as_deref().unwrap_or_default();
    let sources = FileSources {
        root: Path::new(ctx.root),
    };
    let val = doc_tools::flags_from_sources(ctx.root, &sources, ctx.ingest, ctx.interner);
    let entries = flag_entries(gate, &val)?;
    let reach = flip_reach(&entries, ctx.ingest, ctx.graph, &ctx.pr.ranks, ctx.interner);
    render_flip_xml(gate, ctx.root, &entries, &reach, ctx.page_request(), out)
}

/// The `--flags` rows named exactly `gate`. An empty or unknown gate is an error, never an empty success.
pub fn flag_entries<'a>(gate: &str, flags_val: &'a Value) -> io::Result<Vec<&'a Value>> {
    if gate.trim().is_empty() {
        return Err(io::Error::other(
            "ripwire-rs: --flip needs a gate name from the --flags table",
        ));
    }
    let entries: Vec<&Value> = flags_val["flags"].as_array().map_or_else(Vec::new, |arr| {
        arr.iter()
            .filter(|f| f["name"].as_str() == Some(gate))
            .collect()
    });
    if entries.is_empty() {
        return Err(io::Error::other(format!(
            "ripwire-rs: no flag named '{gate}' in the --flags table"
        )));
    }
    Ok(entries)
}

/// What reads a flag and what reaches those reads over the static call graph.
#[derive(Default)]
pub struct FlipReach {
    /// `(path, line, name)` of each indexed symbol that reads the flag.
    pub readers: Vec<(String, u32, String)>,
    /// Reads outside any indexed symbol (manifests, config, module scope); only their file is known.
    pub unscoped_reads: usize,
    /// Transitive callers of the reading symbols, nearest first.
    pub callers: Vec<ImpactNode>,
    /// Test files that reach the reading symbols or sit beside a reading file.
    pub tests: Vec<AffectedTest>,
    /// Distinct files holding a reader, a read, or a caller.
    pub files: usize,
}

pub fn flip_reach(
    entries: &[&Value],
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
) -> FlipReach {
    let mut reader_ids: BTreeSet<NodeId> = BTreeSet::new();
    let mut read_paths: BTreeSet<String> = BTreeSet::new();
    let mut unscoped_reads = 0usize;
    for entry in entries {
        for read in entry["reads"].as_array().into_iter().flatten() {
            let (Some(path), Some(line)) = (read["path"].as_str(), read["line"].as_u64()) else {
                continue;
            };
            read_paths.insert(path.to_string());
            let enclosing = resolve_symbol_chain_at(path, line as u32, ingest, interner)
                .into_iter()
                .rev()
                .find(|s| ingest.sym_file_path(s, interner) == path);
            match enclosing {
                Some(sym) => {
                    reader_ids.insert(sym.id);
                }
                None => unscoped_reads += 1,
            }
        }
    }
    let seeds: Vec<NodeId> = reader_ids.iter().copied().collect();
    let callers = transitive_impact_multi(&seeds, ingest, graph, ranks, interner, None);
    let read_refs: Vec<&str> = read_paths.iter().map(String::as_str).collect();
    let tests = crate::gate::affected::find_affected_tests_for_symbols(
        &seeds,
        &read_refs,
        ingest,
        graph,
        interner,
    )
    .tests;
    let readers: Vec<(String, u32, String)> = reader_ids
        .iter()
        .filter_map(|&id| ingest.sym(id))
        .map(|s| {
            (
                ingest.sym_file_path(s, interner).to_string(),
                s.line,
                interner.resolve(&s.name).to_string(),
            )
        })
        .collect();
    let mut files: BTreeSet<&str> = read_paths.iter().map(String::as_str).collect();
    files.extend(readers.iter().map(|(p, _, _)| p.as_str()));
    files.extend(callers.iter().map(|c| c.path.as_str()));
    FlipReach {
        files: files.len(),
        readers,
        unscoped_reads,
        callers,
        tests,
    }
}

pub fn render_flip_xml<W: Write>(
    gate: &str,
    root: &str,
    entries: &[&Value],
    reach: &FlipReach,
    page: PageRequest,
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<flip schema=\"ripwire-rs.flip/v2\" gate=\"{}\" root=\"{}\">",
        gate.xml_escaped(),
        root.xml_escaped()
    )?;
    for f in entries {
        let name = f["name"].as_str().unwrap_or("");
        let p = f["path"].as_str().unwrap_or("").xml_escaped();
        let l = f["line"].as_u64().unwrap_or(1);
        let kind = f["kind"].as_str().unwrap_or("").xml_escaped();
        let reads = f["reads_count"].as_u64().unwrap_or(0);
        writeln!(
            out,
            "  <site p=\"{p}\" l=\"{l}\" gate=\"{}\" kind=\"{kind}\" reads=\"{reads}\"/>",
            name.xml_escaped()
        )?;
    }
    writeln!(
        out,
        "  <reach readers=\"{}\" unscoped_reads=\"{}\" files=\"{}\">",
        reach.readers.len(),
        reach.unscoped_reads,
        reach.files,
    )?;
    for (p, l, n) in &reach.readers {
        writeln!(
            out,
            "    <reader p=\"{}\" l=\"{l}\" n=\"{}\"/>",
            p.xml_escaped(),
            n.xml_escaped()
        )?;
    }
    let callers = Paged::new(&reach.callers, page, |c| text_bytes(&[c.path.as_str()]));
    writeln!(
        out,
        "    <callers n=\"{}\"{}>",
        reach.callers.len(),
        callers.window.xml_attrs()
    )?;
    for c in callers.rows() {
        writeln!(
            out,
            "      <caller p=\"{}\" l=\"{}\" n=\"{}\" hops=\"{}\" tested=\"{}\"/>",
            c.path.xml_escaped(),
            c.line,
            c.name.xml_escaped(),
            c.hops,
            usize::from(c.tested)
        )?;
    }
    writeln!(out, "    </callers>")?;
    let tests = Paged::new(&reach.tests, page, |t| text_bytes(&[t.path.as_str()]));
    writeln!(
        out,
        "    <tests n=\"{}\"{}>",
        reach.tests.len(),
        tests.window.xml_attrs()
    )?;
    for t in tests.rows() {
        writeln!(
            out,
            "      <test p=\"{}\" hops=\"{}\"/>",
            t.path.xml_escaped(),
            t.hops
        )?;
    }
    writeln!(out, "    </tests>")?;
    writeln!(out, "  </reach>")?;
    writeln!(out, "</flip>")
}
