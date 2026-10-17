use crate::domain::{IngestResult, Interner, NodeId, Symbol};
use crate::graph::Graph;
use crate::rank::PageRankResult;
use crate::serialize::escape::{XmlEscapeExt, escape_xml_write};
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
    pub emit_legend: bool,
    pub legend_full: bool,
    pub map_diff_at: Option<String>,
    pub metrics: bool,
}

impl Default for SerializeOptions {
    fn default() -> Self {
        Self {
            root: ".".to_string(),
            top_k: None,
            include_pr: true,
            pack_budget_bytes: Some(DEFAULT_PACK_BUDGET_BYTES),
            rank_by: None,
            emit_legend: false,
            legend_full: false,
            map_diff_at: None,
            metrics: false,
        }
    }
}

/// Metadata and callee context for serializing a single symbol row.
pub struct SymbolRowContext<'a> {
    pub sym: &'a Symbol,
    pub sym_name: &'a str,
    pub rank: f64,
    pub amb: u32,
    pub callees: &'a [(NodeId, f32, bool)],
    pub include_pr: bool,
    pub metrics: bool,
    pub in_degree: usize,
    pub out_degree: usize,
    pub amp: usize,
}

/// Write the root `<r ...>` opening element.
/// Metadata for writing root XML element.
pub struct RootElementContext<'a> {
    pub root: &'a str,
    pub files: usize,
    pub symbols: usize,
    pub edges: usize,
    pub shown: usize,
    pub pr_iters: u32,
    pub converged: bool,
    pub rank_by: Option<&'a str>,
    pub ambiguous: usize,
    pub unresolved: usize,
    pub schema: Option<&'a str>,
}

/// Write the root `<r ...>` opening element.
pub fn write_root_open<W: Write>(w: &mut W, ctx: &RootElementContext<'_>) -> io::Result<()> {
    write_root_open_with_rank_by(w, ctx)
}

pub fn write_root_open_with_rank_by<W: Write>(
    w: &mut W,
    ctx: &RootElementContext<'_>,
) -> io::Result<()> {
    let schema = ctx.schema.unwrap_or("ripwire.map/v1");
    write!(w, "<r schema=\"{schema}\" root=\"")?;
    escape_xml_write(w, ctx.root)?;
    write!(
        w,
        "\" files=\"{}\" symbols=\"{}\" edges=\"{}\" shown=\"{}\" ambiguous=\"{}\" unresolved=\"{}\"",
        ctx.files, ctx.symbols, ctx.edges, ctx.shown, ctx.ambiguous, ctx.unresolved
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
    let close: &[u8] = if crate::gate::affected::is_test_path(path) {
        b"\" layer=\"test\">"
    } else {
        b"\">"
    };
    w.write_all(b"<f p=\"")?;
    escape_xml_write(w, path)?;
    w.write_all(close)
}

fn write_symbol_metrics_tag<W: Write>(w: &mut W, ctx: &SymbolRowContext<'_>) -> io::Result<()> {
    write!(
        w,
        " in=\"{}\" out=\"{}\" cx=\"{}\" ccx=\"{}\" loc=\"{}\" params=\"{}\" nest=\"{}\" amp=\"{}\"",
        ctx.in_degree,
        ctx.out_degree,
        ctx.sym.cx,
        ctx.sym.ccx,
        ctx.sym.loc,
        ctx.sym.params,
        ctx.sym.max_nest,
        ctx.amp,
    )
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
    if ctx.metrics {
        write_symbol_metrics_tag(w, ctx)?;
    }
    w.write_all(b">")?;
    write_callees_tags(w, ctx.callees, interner, symbols)?;
    w.write_all(b"</s>")
}

fn write_callees_tags<W: Write>(
    w: &mut W,
    callees: &[(NodeId, f32, bool)],
    interner: &Interner,
    symbols: &[Symbol],
) -> io::Result<()> {
    let mut seen_callees = std::collections::HashSet::new();
    for &(callee_id, _, is_split) in callees {
        let callee_sym = &symbols[callee_id as usize];
        let callee_name = interner.resolve(&callee_sym.name);
        if seen_callees.insert(callee_name) {
            write_callee_tag(w, callee_name, is_split)?;
        }
    }
    Ok(())
}

fn write_callee_tag<W: Write>(w: &mut W, name: &str, is_split: bool) -> io::Result<()> {
    write!(
        w,
        "<c n=\"{}\"{}>",
        name.xml_escaped(),
        if is_split { " prov=\"split\"/" } else { "/" }
    )
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

    let mut order: Vec<NodeId> = (0..s as NodeId)
        .filter(|&id| symbols[id as usize].kind != crate::domain::SymKind::Section)
        .collect();
    order.sort_by(|&a, &b| {
        let ra = ranks.get(a as usize).copied().unwrap_or(0.0);
        let rb = ranks.get(b as usize).copied().unwrap_or(0.0);
        rb.total_cmp(&ra).then_with(|| a.cmp(&b))
    });
    let keep = keep.min(order.len());

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

pub const SCHEMA_COMMENT: &[u8] = b"<!-- ripwire map schema=ripwire.map/v1: ranked symbol map: <f p= layer=> groups <s t= n= sc= k= amb=> rows (k= rank), <c n=> resolved callees; the header comment is data. est_tokens=: price as emitted (an upper bound under compact). root=: p= relative to it. pr_iters=N: PageRank iterations. declined=K: K calls left unbound (no evidence chose one def). prov=scip|binding|import|split|final-segment: how that <c> edge bound (absent: one unique name); split = one arm of an amb= pick; final-segment = a qualified type matched by last name only. files=/symbols=: files and symbols indexed; edges= distinct call edges; shown= symbols printed, a merged row counting each def; ambiguous= calls split over several defs, corpus-wide; unresolved= calls with in-tree evidence and no edge (defs all language-filtered, or import/pointer binding refused); order= rows by rank (important-first, important-last; (auto:fill) = flipped past a size threshold) or by path (stable). unindexed=ext:N: N text files of that extension no grammar reads (6 extensions at most). sc=: enclosing scope; the full id is p::sc::n (p= of the row or its <f>) and selectors take it. amb=K: K calls split over several defs.";
pub const SCHEMA_COMMENT_COMPACT: &[u8] = b"<!-- ripwire map schema=ripwire.map/v1: <f p= layer=> <s t= n= sc= k= amb=> <c n=> (dictv=80b7e589253a8e76; full dict: --legend-dict)";

fn build_symbol_row_context<'a>(
    sym: &'a Symbol,
    sym_id: NodeId,
    ctx: &'a StreamXmlContext<'_>,
    callees: &'a [(NodeId, f32, bool)],
    include_pr: bool,
    metrics: bool,
) -> SymbolRowContext<'a> {
    let sym_name = ctx.interner.resolve(&sym.name);
    let rank = ctx.ranks.get(sym_id as usize).copied().unwrap_or(0.0);
    let amb = ctx.graph.ambiguity(sym_id);
    let (in_degree, out_degree) = if metrics {
        (ctx.graph.in_degree(sym_id), ctx.graph.out_degree(sym_id))
    } else {
        (0, 0)
    };
    SymbolRowContext {
        sym,
        sym_name,
        rank,
        amb,
        callees,
        include_pr,
        metrics,
        in_degree,
        out_degree,
        amp: in_degree,
    }
}

fn serialize_symbol_buckets(
    ctx: &StreamXmlContext<'_>,
    file_order: &[u32],
    buckets: &[Vec<NodeId>],
    budget: usize,
    include_pr: bool,
    metrics: bool,
) -> io::Result<(Vec<u8>, usize)> {
    let mut body_buf = Vec::with_capacity(budget.min(DEFAULT_STREAM_BUFFER_CAPACITY));
    let mut file_buf = Vec::new();
    let mut symbols_shown = 0usize;

    for &f in file_order {
        if body_buf.len() >= budget {
            break;
        }
        let file_info = &ctx.ingest.files[f as usize];
        let file_path = ctx.interner.resolve(&file_info.path);
        file_buf.clear();
        write_file_header(&mut file_buf, file_path)?;

        let mut file_has_symbols = false;
        for &sym_id in &buckets[f as usize] {
            if body_buf.len() + file_buf.len() >= budget {
                break;
            }
            let sym = &ctx.ingest.symbols[sym_id as usize];
            let callees: Vec<(NodeId, f32, bool)> = ctx.graph.callees_with_prov(sym_id).collect();
            let row_ctx = build_symbol_row_context(sym, sym_id, ctx, &callees, include_pr, metrics);

            write_symbol_row(&mut file_buf, &row_ctx, ctx.interner, &ctx.ingest.symbols)?;
            symbols_shown += 1;
            file_has_symbols = true;
        }
        if file_has_symbols {
            file_buf.extend_from_slice(b"</f>");
            body_buf.extend_from_slice(&file_buf);
        }
    }

    Ok((body_buf, symbols_shown))
}

/// Stream minified XML with custom ranks and budget enforcement.
fn resolve_cli_map_schema(options: &SerializeOptions) -> &'static str {
    if options.map_diff_at.is_some() {
        "ripwire.map-diff/v1"
    } else if options.metrics {
        "ripwire.metrics/v1"
    } else {
        "ripwire.map/v1"
    }
}

fn should_emit_data_sections_cut(options: &SerializeOptions, data_sections_cut: usize) -> bool {
    let k_lim = options.top_k.unwrap_or(200);
    data_sections_cut > 0
        && options.map_diff_at.is_none()
        && k_lim >= 100
        && options
            .pack_budget_bytes
            .is_none_or(|b| b >= DEFAULT_PACK_BUDGET_BYTES)
}

fn write_cli_root_open<W: Write>(
    w: &mut W,
    options: &SerializeOptions,
    pr_iters: u32,
    est_tokens: usize,
    data_sections_cut: usize,
) -> io::Result<()> {
    w.write_all(b"<r")?;
    if !options.legend_full {
        let schema = resolve_cli_map_schema(options);
        write!(w, " schema=\"{schema}\"")?;
    }
    if let Some(at) = &options.map_diff_at {
        write!(w, " at=\"{at}\"")?;
    }
    w.write_all(b" root=\"")?;
    escape_xml_write(w, &options.root)?;
    w.write_all(b"\"")?;
    if let Some(rb) = &options.rank_by {
        return write!(w, " rank_by=\"{rb}\" est_tokens=\"{est_tokens}\">");
    }
    write!(w, " est_tokens=\"{est_tokens}\" pr_iters=\"{pr_iters}\"")?;
    if should_emit_data_sections_cut(options, data_sections_cut) {
        let k_lim = options.top_k.unwrap_or(200);
        write!(
            w,
            " data_sections_cut=\"{data_sections_cut}\" next=\"--graph-query=&apos;kind(all,sec)&apos; --offset=0 --limit={k_lim}\""
        )?;
    }
    w.write_all(b">")
}

/// Stream minified XML with custom ranks and budget enforcement.
pub fn serialize_xml_with_ranks<W: Write>(
    ctx: &StreamXmlContext<'_>,
    writer: &mut W,
    options: &SerializeOptions,
) -> io::Result<()> {
    let (file_order, buckets) = bucket_symbols_by_rank(
        &ctx.ingest.symbols,
        ctx.ingest.files.len(),
        ctx.ranks,
        options.top_k,
    );

    let budget = options.pack_budget_bytes.unwrap_or(usize::MAX);
    let (body_buf, symbols_shown) = serialize_symbol_buckets(
        ctx,
        &file_order,
        &buckets,
        budget,
        options.include_pr,
        options.metrics,
    )?;

    let buf = BufWriter::with_capacity(DEFAULT_STREAM_BUFFER_CAPACITY, writer);
    let mut counter = CountingWriter::new(buf);
    let ambiguous = ctx.graph.amb_out.iter().map(|&a| a as usize).sum::<usize>();
    if options.emit_legend || options.metrics {
        let header_prefix = if options.legend_full || options.metrics {
            SCHEMA_COMMENT
        } else {
            SCHEMA_COMMENT_COMPACT
        };
        counter.write_all(header_prefix)?;
        write!(
            counter,
            " files={} symbols={} edges={} shown=\"{}\" ambiguous={} unresolved=0 -->",
            ctx.ingest.file_count(),
            ctx.ingest.symbol_count(),
            ctx.graph.edge_count(),
            symbols_shown,
            ambiguous
        )?;
        let est_tokens = (body_buf.len() + 1200) * 2 / 5;
        let data_sections_cut = ctx
            .ingest
            .symbols
            .iter()
            .filter(|s| s.kind == crate::domain::SymKind::Section)
            .count();
        write_cli_root_open(
            &mut counter,
            options,
            ctx.pr_iters,
            est_tokens,
            data_sections_cut,
        )?;
    } else {
        let root_ctx = RootElementContext {
            root: &options.root,
            files: ctx.ingest.file_count(),
            symbols: ctx.ingest.symbol_count(),
            edges: ctx.graph.edge_count(),
            shown: symbols_shown,
            pr_iters: ctx.pr_iters,
            converged: ctx.pr_converged,
            rank_by: options.rank_by.as_deref(),
            ambiguous,
            unresolved: ctx.graph.total_unresolved(),
            schema: None,
        };
        write_root_open(&mut counter, &root_ctx)?;
    }
    counter.write_all(&body_buf)?;
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

        assert!(xml.starts_with("<r schema=\"ripwire.map/v1\" root=\".\" files=\"1\" symbols=\"2\" edges=\"1\" shown=\"2\" ambiguous=\"0\" unresolved=\"0\" pr_iters=\"12\">"));
        assert!(xml.contains("<f p=\"src/main.rs\">"));
        assert!(xml.contains("<s t=\"fn\" n=\"run\" k=\"0.6500\"></s>"));
        assert!(xml.contains("<s t=\"fn\" n=\"start\" k=\"0.3500\"><c n=\"run\"/></s>"));
        assert!(xml.ends_with("</f></r>\n"));
    }
}
