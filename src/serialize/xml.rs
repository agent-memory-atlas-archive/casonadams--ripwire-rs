use crate::domain::{IngestResult, Interner, NodeId, Symbol};
use crate::graph::Graph;
use crate::rank::PageRankResult;
use crate::serialize::escape::escape_xml_write;
use std::io::{self, BufWriter, Write};

pub const DEFAULT_STREAM_BUFFER_CAPACITY: usize = 64 * 1024;
pub const DEFAULT_PACK_BUDGET_BYTES: usize = 64 * 1024;

/// Configuration options for streaming XML serialization.
#[derive(Debug, Clone, PartialEq)]
pub struct SerializeOptions {
    pub root: String,
    pub top_k: Option<usize>,
    pub include_pr: bool,
    pub pack_budget_bytes: Option<usize>,
    pub rank_by: Option<String>,
}

impl Default for SerializeOptions {
    fn default() -> Self {
        Self {
            root: ".".into(),
            top_k: None,
            include_pr: true,
            pack_budget_bytes: Some(DEFAULT_PACK_BUDGET_BYTES),
            rank_by: None,
        }
    }
}

/// Metadata and callee context for serializing a single symbol row.
pub struct SymbolRowContext<'a> {
    pub sym: &'a Symbol,
    pub sym_name: &'a str,
    pub rank: f64,
    pub amb: u32,
    pub callees: &'a [(NodeId, f32)],
    pub include_pr: bool,
}

/// Write the root `<r ...>` opening element.
/// Metadata for writing root XML element.
pub struct RootElementContext<'a> {
    pub root: &'a str,
    pub files: usize,
    pub symbols: usize,
    pub edges: usize,
    pub pr_iters: u32,
    pub converged: bool,
    pub rank_by: Option<&'a str>,
}

/// Write the root `<r ...>` opening element.
pub fn write_root_open<W: Write>(
    w: &mut W,
    root: &str,
    files: usize,
    symbols: usize,
    edges: usize,
    pr_iters: u32,
    converged: bool,
) -> io::Result<()> {
    write_root_open_with_rank_by(
        w,
        &RootElementContext {
            root,
            files,
            symbols,
            edges,
            pr_iters,
            converged,
            rank_by: None,
        },
    )
}

pub fn write_root_open_with_rank_by<W: Write>(
    w: &mut W,
    ctx: &RootElementContext<'_>,
) -> io::Result<()> {
    w.write_all(b"<r schema=\"rwire.map/v1\" root=\"")?;
    escape_xml_write(w, ctx.root)?;
    write!(
        w,
        "\" files=\"{}\" symbols=\"{}\" edges=\"{}\"",
        ctx.files, ctx.symbols, ctx.edges
    )?;
    if let Some(rb) = ctx.rank_by {
        write!(w, " rank_by=\"{}\"", rb)?;
    } else {
        write!(w, " pr_iters=\"{}\"", ctx.pr_iters)?;
        if !ctx.converged {
            w.write_all(b" pr_converged=\"0\"")?;
        }
    }
    w.write_all(b">")
}

/// Counting writer to enforce streaming pack budgets.
pub struct CountingWriter<W> {
    inner: W,
    pub count: usize,
}

impl<W: Write> CountingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner, count: 0 }
    }
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.count += n;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Write the file opening element `<f p="...">`.
pub fn write_file_header<W: Write>(w: &mut W, path: &str) -> io::Result<()> {
    w.write_all(b"<f p=\"")?;
    escape_xml_write(w, path)?;
    w.write_all(b"\">")
}

/// Write a single symbol row and its outgoing call edges.
pub fn write_symbol_row<W: Write>(
    w: &mut W,
    ctx: &SymbolRowContext<'_>,
    interner: &Interner,
    symbols: &[Symbol],
) -> io::Result<()> {
    w.write_all(b"<s t=\"")?;
    w.write_all(ctx.sym.kind.tag().as_bytes())?;
    w.write_all(b"\" n=\"")?;
    escape_xml_write(w, ctx.sym_name)?;
    w.write_all(b"\"")?;

    if let Some(sc_spur) = ctx.sym.scope {
        let sc = interner.resolve(&sc_spur);
        if !sc.is_empty() {
            w.write_all(b" sc=\"")?;
            escape_xml_write(w, sc)?;
            w.write_all(b"\"")?;
        }
    }

    if ctx.include_pr {
        write!(w, " k=\"{:.4}\"", ctx.rank)?;
    }
    if ctx.amb > 0 {
        write!(w, " amb=\"{}\"", ctx.amb)?;
    }
    w.write_all(b">")?;

    let mut seen_callees = std::collections::HashSet::new();
    for &(callee_id, _) in ctx.callees {
        let callee_sym = &symbols[callee_id as usize];
        let callee_name = interner.resolve(&callee_sym.name);
        if seen_callees.insert(callee_name) {
            w.write_all(b"<c n=\"")?;
            escape_xml_write(w, callee_name)?;
            w.write_all(b"\"/>")?;
        }
    }
    w.write_all(b"</s>")
}

/// Compute symbol rank ordering and bucket symbols into files.
pub fn bucket_symbols_by_rank(
    symbols: &[Symbol],
    files_len: usize,
    ranks: &[f64],
    top_k: Option<usize>,
) -> (Vec<u32>, Vec<Vec<NodeId>>) {
    let s = symbols.len();
    let keep = top_k.unwrap_or(s).min(s);

    let mut order: Vec<NodeId> = (0..s as NodeId).collect();
    order.sort_by(|&a, &b| {
        let ra = ranks.get(a as usize).copied().unwrap_or(0.0);
        let rb = ranks.get(b as usize).copied().unwrap_or(0.0);
        rb.total_cmp(&ra).then_with(|| a.cmp(&b))
    });

    let mut buckets: Vec<Vec<NodeId>> = vec![Vec::new(); files_len];
    let mut file_order: Vec<u32> = Vec::new();
    let mut seen_files = vec![false; files_len];

    for &id in &order[..keep] {
        let f = symbols[id as usize].file_id;
        if (f as usize) < seen_files.len() && !seen_files[f as usize] {
            seen_files[f as usize] = true;
            file_order.push(f);
        }
        if (f as usize) < buckets.len() {
            buckets[f as usize].push(id);
        }
    }

    (file_order, buckets)
}

/// Stream minified, token-dense XML directly through a buffered writer to stdout or output.
/// Context for streaming XML serialization with custom ranks.
pub struct StreamXmlContext<'a> {
    pub ingest: &'a IngestResult,
    pub graph: &'a Graph,
    pub ranks: &'a [f64],
    pub pr_iters: u32,
    pub pr_converged: bool,
    pub interner: &'a Interner,
}

pub fn serialize_xml<W: Write>(
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    writer: &mut W,
    options: &SerializeOptions,
) -> io::Result<()> {
    serialize_xml_with_ranks(
        &StreamXmlContext {
            ingest,
            graph,
            ranks: &pr.ranks,
            pr_iters: pr.iterations,
            pr_converged: pr.converged,
            interner,
        },
        writer,
        options,
    )
}

/// Stream minified XML with custom ranks and budget enforcement.
pub fn serialize_xml_with_ranks<W: Write>(
    ctx: &StreamXmlContext<'_>,
    writer: &mut W,
    options: &SerializeOptions,
) -> io::Result<()> {
    let buf = BufWriter::with_capacity(DEFAULT_STREAM_BUFFER_CAPACITY, writer);
    let mut counter = CountingWriter::new(buf);

    if let Some(rb) = options.rank_by.as_deref() {
        write_root_open_with_rank_by(
            &mut counter,
            &RootElementContext {
                root: &options.root,
                files: ctx.ingest.file_count(),
                symbols: ctx.ingest.symbol_count(),
                edges: ctx.graph.edge_count(),
                pr_iters: ctx.pr_iters,
                converged: ctx.pr_converged,
                rank_by: Some(rb),
            },
        )?;
    } else {
        write_root_open(
            &mut counter,
            &options.root,
            ctx.ingest.file_count(),
            ctx.ingest.symbol_count(),
            ctx.graph.edge_count(),
            ctx.pr_iters,
            ctx.pr_converged,
        )?;
    }

    let (file_order, buckets) = bucket_symbols_by_rank(
        &ctx.ingest.symbols,
        ctx.ingest.files.len(),
        ctx.ranks,
        options.top_k,
    );

    let budget = options.pack_budget_bytes.unwrap_or(usize::MAX);

    for f in file_order {
        if counter.count >= budget {
            break;
        }
        let file_info = &ctx.ingest.files[f as usize];
        let file_path = ctx.interner.resolve(&file_info.path);
        write_file_header(&mut counter, file_path)?;

        for &sym_id in &buckets[f as usize] {
            if counter.count >= budget {
                break;
            }
            let sym = &ctx.ingest.symbols[sym_id as usize];
            let sym_name = ctx.interner.resolve(&sym.name);
            let rank = ctx.ranks.get(sym_id as usize).copied().unwrap_or(0.0);
            let amb = ctx.graph.ambiguity(sym_id);
            let callees: Vec<(NodeId, f32)> = ctx.graph.callees(sym_id).collect();

            let row_ctx = SymbolRowContext {
                sym,
                sym_name,
                rank,
                amb,
                callees: &callees,
                include_pr: options.include_pr,
            };

            write_symbol_row(&mut counter, &row_ctx, ctx.interner, &ctx.ingest.symbols)?;
        }

        counter.write_all(b"</f>")?;
    }

    counter.write_all(b"</r>\n")?;
    counter.flush()
}

/// Serialize to a minified XML string in memory (primarily for unit tests and inspection).
#[must_use]
pub fn serialize_xml_to_string(
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    options: &SerializeOptions,
) -> String {
    let mut out = Vec::new();
    serialize_xml(ingest, graph, pr, interner, &mut out, options)
        .expect("in-memory serialization should never fail");
    String::from_utf8(out).expect("valid utf-8 xml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FileInfo, Lang, RefRole, Reference, SymKind};

    #[test]
    fn test_serialize_xml_structure() {
        let interner = Interner::new();
        let f0 = FileInfo {
            id: 0,
            path: interner.intern("src/main.rs"),
            lang: Lang::Rust,
            line_count: 20,
            byte_size: 200,
        };

        let sym_a = Symbol::new(
            0,
            interner.intern("start"),
            interner.intern("c_start"),
            SymKind::Function,
            Lang::Rust,
            0,
            5,
        );
        let sym_b = Symbol::new(
            1,
            interner.intern("run"),
            interner.intern("c_run"),
            SymKind::Function,
            Lang::Rust,
            0,
            10,
        );

        let ref_ab = Reference::new(0, interner.intern("run"), 0, 6, Lang::Rust, RefRole::Call);

        let mut ingest = IngestResult::new();
        ingest.files.push(f0);
        ingest.symbols.extend_from_slice(&[sym_a, sym_b]);
        ingest.references.push(ref_ab);

        let graph = Graph::build(&ingest, &interner);
        let pr = PageRankResult {
            iterations: 12,
            converged: true,
            ranks: vec![0.35, 0.65],
        };

        let xml = serialize_xml_to_string(
            &ingest,
            &graph,
            &pr,
            &interner,
            &SerializeOptions::default(),
        );

        assert!(xml.starts_with("<r schema=\"rwire.map/v1\" root=\".\" files=\"1\" symbols=\"2\" edges=\"1\" pr_iters=\"12\">"));
        assert!(xml.contains("<f p=\"src/main.rs\">"));
        assert!(xml.contains("<s t=\"fn\" n=\"run\" k=\"0.6500\"></s>"));
        assert!(xml.contains("<s t=\"fn\" n=\"start\" k=\"0.3500\"><c n=\"run\"/></s>"));
        assert!(xml.ends_with("</f></r>\n"));
    }
}
