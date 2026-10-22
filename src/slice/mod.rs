pub mod arise;
pub mod flow;

pub use arise::*;
pub use flow::*;

use crate::domain::{IngestResult, Interner};
use crate::ingest::LanguageRegistry;
use crate::languages::SyntaxKinds;
use tree_sitter::{Node, Parser};

/// Split target selector such as `symbol:var` or `path::sym:var` into symbol and variable.
#[must_use]
pub fn parse_slice_target(target: &str) -> (&str, Option<&str>) {
    if let Some((sym, var)) = target.rsplit_once(':')
        && !sym.is_empty()
        && !var.is_empty()
        && !sym.ends_with(':')
    {
        return (sym, Some(var));
    }
    (target, None)
}

fn collect_nodes_in_span<'a>(
    node: Node<'a>,
    start_byte: usize,
    end_byte: usize,
    out: &mut Vec<Node<'a>>,
) {
    if node.end_byte() <= start_byte || node.start_byte() >= end_byte {
        return;
    }
    if node.start_byte() >= start_byte && node.end_byte() <= end_byte {
        out.push(node);
        return;
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_nodes_in_span(child, start_byte, end_byte, out);
        }
    }
}

fn collect_span_variables(
    root: Node<'_>,
    start_byte: usize,
    end_byte: usize,
    source_bytes: &[u8],
    kinds: &SyntaxKinds,
) -> Vec<VariableOcc> {
    let mut nodes = Vec::new();
    collect_nodes_in_span(root, start_byte, end_byte, &mut nodes);
    let mut occs = Vec::new();
    for n in nodes {
        occs.extend(collect_symbol_variables(n, source_bytes, None, kinds));
    }
    occs.sort_by_key(|o| o.start_byte);
    occs.dedup_by_key(|o| o.start_byte);
    occs
}

fn list_span_locals(
    root: Node<'_>,
    start_byte: usize,
    end_byte: usize,
    source_bytes: &[u8],
    kinds: &SyntaxKinds,
) -> Vec<(String, u32, OccRole)> {
    if start_byte == 0 && end_byte >= source_bytes.len() {
        return list_sliceable_locals(root, source_bytes, kinds);
    }
    collect_unique_def_locals(collect_span_variables(
        root,
        start_byte,
        end_byte,
        source_bytes,
        kinds,
    ))
}

struct SliceReportContext<'a> {
    request: &'a SliceRequest<'a>,
    symbol: &'a crate::domain::Symbol,
    sym_name: &'a str,
    path_line: &'a str,
}

impl SliceReportContext<'_> {
    fn at_attr(&self) -> String {
        self.request
            .at
            .map(|value| format!(" at=\"{}\"", crate::serialize::escape_xml_str(value)))
            .unwrap_or_default()
    }
}

fn render_locals_xml(report: &SliceReportContext<'_>, locals: &[(String, u32, OccRole)]) -> String {
    let kind_tag = report.symbol.kind.tag();
    let lang_tag = report.symbol.lang.tag();
    use crate::serialize::escape_xml_str;
    let at_attr = report.at_attr();
    let root = escape_xml_str(report.request.root);
    let sym_name = escape_xml_str(report.sym_name);
    let path_line = escape_xml_str(report.path_line);
    let mut out = format!(
        "<!-- ripwire-rs slice schema=ripwire-rs.slice/v1: name-based def-use rows of one variable in one definition. -->\n<slice schema=\"ripwire-rs.slice/v1\" sym=\"{sym_name}\" p=\"{path_line}\" t=\"{kind_tag}\" lang=\"{lang_tag}\" vars=\"{}\"{at_attr} root=\"{root}\" counts=\"as-classified\">\n",
        locals.len()
    );
    for (name, line, role) in locals {
        out.push_str(&format!(
            "  <v n=\"{}\" l=\"{line}\" t=\"{role}\" />\n",
            escape_xml_str(name)
        ));
    }
    out.push_str("</slice>\n");
    out
}

struct FlowRenderCtx<'a> {
    report: &'a SliceReportContext<'a>,
    var_name: &'a str,
    source_bytes: &'a [u8],
}

fn format_step_xml(s: &SliceStep, lines: &[&str]) -> String {
    let line_text = s
        .line
        .checked_sub(1)
        .and_then(|idx| lines.get(idx as usize))
        .map(|l| l.trim())
        .unwrap_or("");
    let from_attrs = if s.from_line > 0 {
        format!(" from=\"{}\" rd=\"{}\"", s.from_line, s.from_line)
    } else {
        String::new()
    };
    if line_text.is_empty() {
        format!(
            "  <s l=\"{}\" k=\"{}\" v=\"{}\" d=\"{}\"{from_attrs} />\n",
            s.line,
            s.role,
            crate::serialize::escape_xml_str(&s.var_name),
            s.depth
        )
    } else {
        format!(
            "  <s l=\"{}\" k=\"{}\" v=\"{}\" d=\"{}\"{from_attrs}><![CDATA[{line_text}]]></s>\n",
            s.line,
            s.role,
            crate::serialize::escape_xml_str(&s.var_name),
            s.depth
        )
    }
}

fn render_flow_xml(ctx: &FlowRenderCtx<'_>, steps: &[SliceStep]) -> String {
    let src_text = String::from_utf8_lossy(ctx.source_bytes);
    let lines: Vec<&str> = src_text.lines().collect();
    let defs_cnt = steps.iter().filter(|s| s.role.is_def()).count();
    let uses_cnt = steps.iter().filter(|s| s.role.is_use()).count();
    use crate::serialize::escape_xml_str;
    let at_attr = ctx.report.at_attr();
    let mut out = format!(
        "<slice schema=\"ripwire-rs.slice/v1\" sym=\"{}\" var=\"{}\" p=\"{}\" t=\"{}\" lang=\"{}\" defs=\"{defs_cnt}\" uses=\"{uses_cnt}\" flow=\"{}\" depth=\"{}\" steps=\"{}\"{at_attr} root=\"{}\" counts=\"as-classified\">\n",
        escape_xml_str(ctx.report.sym_name),
        escape_xml_str(ctx.var_name),
        escape_xml_str(ctx.report.path_line),
        escape_xml_str(ctx.report.symbol.kind.tag()),
        escape_xml_str(ctx.report.symbol.lang.tag()),
        ctx.report.request.flow,
        ctx.report.request.max_depth,
        steps.len(),
        escape_xml_str(ctx.report.request.root)
    );
    for s in steps {
        out.push_str(&format_step_xml(s, &lines));
    }
    out.push_str("</slice>\n");
    out
}

fn collect_slice_candidates<'a>(
    sym_query: &str,
    ingest: &'a IngestResult,
    interner: &'a Interner,
) -> Vec<&'a crate::domain::Symbol> {
    use crate::domain::CorpusView;
    use crate::navigate::SymbolLookup;
    let view = (ingest, interner);
    let ids = view.find_symbols(sym_query);
    let mut cands: Vec<&crate::domain::Symbol> = if !ids.is_empty() {
        ids.into_iter().filter_map(|id| ingest.sym(id)).collect()
    } else {
        ingest
            .symbols
            .iter()
            .filter(|s| {
                interner.resolve(&s.name) == sym_query
                    || interner.resolve(&s.canonical_id) == sym_query
                    || interner.resolve(&s.canonical_id).ends_with(sym_query)
            })
            .collect()
    };
    cands.sort_by_key(|s| {
        let prod = u8::from(!view.is_production_symbol(s));
        let callable = u8::from(!s.is_callable());
        let kind_rank = u8::from(s.kind != crate::domain::SymKind::Function);
        let bodiless = u8::from(!s.has_body());
        (prod, callable, bodiless, kind_rank, s.id)
    });
    cands
}

/// Transport-independent inputs for a slice report.
pub struct SliceRequest<'a> {
    pub root: &'a str,
    pub at: Option<&'a str>,
    pub target: &'a str,
    pub flow: SliceFlowDir,
    pub max_depth: u32,
}

/// Shared repository analysis used to resolve slice definitions.
pub struct SliceContext<'a> {
    pub ingest: &'a IngestResult,
    pub interner: &'a Interner,
    pub registry: &'a LanguageRegistry,
}

fn parse_candidate_variables(
    symbol: &crate::domain::Symbol,
    source_bytes: &[u8],
    registry: &LanguageRegistry,
) -> Result<(tree_sitter::Tree, Vec<VariableOcc>), String> {
    let (ts_lang, _) = registry
        .get(symbol.lang)
        .ok_or_else(|| format!("language not supported for slicing: {:?}", symbol.lang))?;
    let mut parser = Parser::new();
    parser
        .set_language(ts_lang)
        .map_err(|e| format!("failed to initialize parser: {e}"))?;
    let tree = parser
        .parse(source_bytes, None)
        .ok_or_else(|| "failed to parse source file".to_string())?;
    let occs = collect_span_variables(
        tree.root_node(),
        symbol.sig_start_byte as usize,
        symbol.end_byte as usize,
        source_bytes,
        symbol.lang.semantics().syntax_kinds(),
    );
    Ok((tree, occs))
}

fn select_best_slice_candidate<'a>(
    var_opt: Option<&str>,
    candidates: &[&'a crate::domain::Symbol],
    context: &SliceContext<'_>,
    sources: &(impl crate::query::SourceReader + ?Sized),
) -> Result<&'a crate::domain::Symbol, String> {
    let first = candidates[0];
    let Some(var_name) = var_opt.filter(|_| candidates.len() > 1) else {
        return Ok(first);
    };
    let mut first_error = None;
    for &cand in candidates {
        if !cand.is_callable() || !cand.lang.semantics().supports_slice() {
            continue;
        }
        let path = context.ingest.sym_file_path(cand, context.interner);
        let parsed = sources
            .read(path)
            .map_err(|e| format!("cannot read file {path}: {e}"))
            .and_then(|bytes| parse_candidate_variables(cand, &bytes, context.registry));
        match parsed {
            Ok((_, occs)) if occs.iter().any(|o| o.var_name == var_name) => return Ok(cand),
            Ok(_) => {}
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(first)
}

/// Query intra-procedural slicing using only the supplied source provider.
pub fn query_slice(
    request: SliceRequest<'_>,
    context: SliceContext<'_>,
    sources: &(impl crate::query::SourceReader + ?Sized),
) -> Result<String, String> {
    let SliceContext {
        ingest,
        interner,
        registry,
    } = context;
    let (sym_query, var_opt) = parse_slice_target(request.target);
    let candidates = collect_slice_candidates(sym_query, ingest, interner);
    candidates
        .first()
        .ok_or_else(|| format!("symbol not found: {sym_query}"))?;
    let chosen = select_best_slice_candidate(var_opt, &candidates, &context, sources)?;
    if !chosen.lang.semantics().supports_slice() || chosen.kind == crate::domain::SymKind::Section {
        return Err(format!(
            "--slice: slice not served for {} yet — the def-use classification requires a verified per-grammar local variable AST",
            chosen.lang.tag()
        ));
    }
    let file_path = ingest.sym_file_path(chosen, interner);
    let source_bytes = sources
        .read(file_path)
        .map_err(|e| format!("cannot read file {file_path}: {e}"))?;
    let (tree, all_occs) = parse_candidate_variables(chosen, &source_bytes, registry)?;
    let sym_name = interner.resolve(&chosen.name);
    let path_line = format!("{file_path}:{}", chosen.line);

    let report = SliceReportContext {
        request: &request,
        symbol: chosen,
        sym_name,
        path_line: &path_line,
    };

    match var_opt {
        None => {
            let locals = list_span_locals(
                tree.root_node(),
                chosen.sig_start_byte as usize,
                chosen.end_byte as usize,
                &source_bytes,
                chosen.lang.semantics().syntax_kinds(),
            );
            Ok(render_locals_xml(&report, &locals))
        }
        Some(var_name) => {
            let steps =
                compute_dataflow_slice(&all_occs, var_name, request.flow, request.max_depth);
            let ctx = FlowRenderCtx {
                report: &report,
                var_name,
                source_bytes: &source_bytes,
            };
            Ok(render_flow_xml(&ctx, &steps))
        }
    }
}
