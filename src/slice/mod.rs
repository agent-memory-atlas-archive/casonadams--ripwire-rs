pub mod arise;
pub mod flow;

pub use arise::*;
pub use flow::*;

use crate::domain::{IngestResult, Interner};
use crate::ingest::LanguageRegistry;
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
) -> Vec<VariableOcc> {
    let mut nodes = Vec::new();
    collect_nodes_in_span(root, start_byte, end_byte, &mut nodes);
    let mut occs = Vec::new();
    for n in nodes {
        occs.extend(collect_symbol_variables(n, source_bytes, None));
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
) -> Vec<(String, u32, OccRole)> {
    if start_byte == 0 && end_byte >= source_bytes.len() {
        return list_sliceable_locals(root, source_bytes);
    }
    let occs = collect_span_variables(root, start_byte, end_byte, source_bytes);
    let mut seen = std::collections::HashSet::new();
    let mut locals = Vec::new();
    for occ in occs {
        if occ.role.is_def() && seen.insert(occ.var_name.clone()) {
            locals.push((occ.var_name, occ.line, occ.role));
        }
    }
    locals
}

/// Render XML for sliceable locals inventory when no variable was given.
fn render_locals_xml(
    sym_name: &str,
    path_line: &str,
    kind_tag: &str,
    lang_tag: &str,
    locals: &[(String, u32, OccRole)],
) -> String {
    let at_stamp = crate::git::history::compute_git_at_stamp(".").unwrap_or_else(|| "HEAD".into());
    let mut out = format!(
        "<!-- ripwire slice schema=ripwire.slice/v1: name-based def-use rows of one variable in one definition. -->\n<slice schema=\"ripwire.slice/v1\" sym=\"{sym_name}\" p=\"{path_line}\" t=\"{kind_tag}\" lang=\"{lang_tag}\" vars=\"{}\" at=\"{at_stamp}\" root=\".\" counts=\"as-classified\">\n",
        locals.len()
    );
    for (name, line, role) in locals {
        out.push_str(&format!("  <v n=\"{name}\" l=\"{line}\" t=\"{role}\" />\n"));
    }
    out.push_str("</slice>\n");
    out
}

struct FlowRenderCtx<'a> {
    sym_name: &'a str,
    var_name: &'a str,
    path_line: &'a str,
    kind_tag: &'a str,
    lang_tag: &'a str,
    root: &'a str,
    dir: SliceFlowDir,
    depth: u32,
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
            s.line, s.role, s.var_name, s.depth
        )
    } else {
        format!(
            "  <s l=\"{}\" k=\"{}\" v=\"{}\" d=\"{}\"{from_attrs}><![CDATA[{line_text}]]></s>\n",
            s.line, s.role, s.var_name, s.depth
        )
    }
}

/// Render XML for transitive dataflow slice.
fn render_flow_xml(ctx: &FlowRenderCtx<'_>, steps: &[SliceStep]) -> String {
    let src_text = String::from_utf8_lossy(ctx.source_bytes);
    let lines: Vec<&str> = src_text.lines().collect();
    let defs_cnt = steps.iter().filter(|s| s.role.is_def()).count();
    let uses_cnt = steps.iter().filter(|s| s.role.is_use()).count();
    let mut out = format!(
        "<slice schema=\"ripwire.slice/v1\" sym=\"{}\" var=\"{}\" p=\"{}\" t=\"{}\" lang=\"{}\" defs=\"{defs_cnt}\" uses=\"{uses_cnt}\" flow=\"{}\" depth=\"{}\" steps=\"{}\" root=\"{}\" counts=\"as-classified\">\n",
        ctx.sym_name,
        ctx.var_name,
        ctx.path_line,
        ctx.kind_tag,
        ctx.lang_tag,
        ctx.dir,
        ctx.depth,
        steps.len(),
        ctx.root
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

fn read_slice_candidate_bytes(
    root: &str,
    file_path: &str,
    custom_source: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    if let Some(b) = custom_source {
        return Ok(b.to_vec());
    }
    let root_candidate = std::path::Path::new(root).join(file_path);
    std::fs::read(&root_candidate)
        .or_else(|_| std::fs::read(file_path))
        .map_err(|e| format!("cannot read file {file_path}: {e}"))
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
    );
    Ok((tree, occs))
}

fn select_best_slice_candidate<'a>(
    root: &str,
    var_opt: Option<&str>,
    custom_source: Option<&[u8]>,
    candidates: &[&'a crate::domain::Symbol],
    ingest: &'a IngestResult,
    interner: &Interner,
    registry: &LanguageRegistry,
) -> &'a crate::domain::Symbol {
    let first = candidates[0];
    let Some(var_name) = var_opt.filter(|_| candidates.len() > 1 && custom_source.is_none()) else {
        return first;
    };
    for &cand in candidates {
        if !cand.is_callable() || !cand.lang.semantics().supports_slice() {
            continue;
        }
        let fpath = ingest.sym_file_path(cand, interner);
        if let Ok(bytes) = read_slice_candidate_bytes(root, fpath, None)
            && let Ok((_, occs)) = parse_candidate_variables(cand, &bytes, registry)
            && occs.iter().any(|o| o.var_name == var_name)
        {
            return cand;
        }
    }
    first
}

/// Execute intra-procedural def-use variable slicing on a symbol target.
pub fn execute_slice(
    ingest: &IngestResult,
    registry: &LanguageRegistry,
    interner: &Interner,
    target: &str,
    flow_dir: SliceFlowDir,
    max_depth: u32,
    custom_source: Option<&[u8]>,
) -> Result<String, String> {
    execute_slice_with_root(
        ".",
        (ingest, interner),
        registry,
        target,
        flow_dir,
        max_depth,
        custom_source,
    )
}

fn execute_slice_with_root(
    root: &str,
    corpus: (&IngestResult, &Interner),
    registry: &LanguageRegistry,
    target: &str,
    flow_dir: SliceFlowDir,
    max_depth: u32,
    custom_source: Option<&[u8]>,
) -> Result<String, String> {
    let (ingest, interner) = corpus;
    let (sym_query, var_opt) = parse_slice_target(target);
    let candidates = collect_slice_candidates(sym_query, ingest, interner);
    let first = candidates
        .first()
        .copied()
        .ok_or_else(|| format!("symbol not found: {sym_query}"))?;
    if !first.lang.semantics().supports_slice() || first.kind == crate::domain::SymKind::Section {
        return Err(format!(
            "--slice: slice not served for {} yet — the def-use classification requires a verified per-grammar local variable AST",
            first.lang.tag()
        ));
    }

    let chosen = select_best_slice_candidate(
        root,
        var_opt,
        custom_source,
        &candidates,
        ingest,
        interner,
        registry,
    );
    let file_path = ingest.sym_file_path(chosen, interner);
    let source_bytes = read_slice_candidate_bytes(root, file_path, custom_source)?;
    let (tree, all_occs) = parse_candidate_variables(chosen, &source_bytes, registry)?;
    let sym_name = interner.resolve(&chosen.name);
    let path_line = format!("{file_path}:{}", chosen.line);

    match var_opt {
        None => {
            let locals = list_span_locals(
                tree.root_node(),
                chosen.sig_start_byte as usize,
                chosen.end_byte as usize,
                &source_bytes,
            );
            Ok(render_locals_xml(
                sym_name,
                &path_line,
                chosen.kind.tag(),
                chosen.lang.tag(),
                &locals,
            ))
        }
        Some(var_name) => {
            let steps = compute_dataflow_slice(&all_occs, var_name, flow_dir, max_depth);
            let ctx = FlowRenderCtx {
                sym_name,
                var_name,
                path_line: &path_line,
                kind_tag: chosen.kind.tag(),
                lang_tag: chosen.lang.tag(),
                root,
                dir: flow_dir,
                depth: max_depth,
                source_bytes: &source_bytes,
            };
            Ok(render_flow_xml(&ctx, &steps))
        }
    }
}

/// Helper entry point for executing slicing from CLI arguments.
pub fn execute_slice_command(
    root: &str,
    target: Option<&str>,
    flow_str: &str,
) -> std::io::Result<()> {
    execute_slice_command_with_depth(root, target, flow_str, 8)
}

pub fn execute_slice_command_with_depth(
    root: &str,
    target: Option<&str>,
    flow_str: &str,
    max_depth: u32,
) -> std::io::Result<()> {
    let Some(target) = target else {
        eprintln!("ripwire: target required (e.g. ripwire slice sym:var)");
        return Ok(());
    };

    let (ingest, _graph, _pr, interner) = crate::cli::load_repo_analysis(root)?;
    let registry = LanguageRegistry::new().map_err(|e| std::io::Error::other(e.to_string()))?;
    let flow_dir = flow_str.parse::<SliceFlowDir>().unwrap_or_default();
    match execute_slice_with_root(
        root,
        (&ingest, &interner),
        &registry,
        target,
        flow_dir,
        max_depth,
        None,
    ) {
        Ok(out) => {
            use std::io::Write;
            let mut stdout_lock = std::io::stdout().lock();
            let mut writer = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
            writer.write_all(out.as_bytes())?;
            writer.flush()
        }
        Err(e) => {
            eprintln!("ripwire: {e}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Lang;
    use crate::ingest::ingest_in_memory_files;

    #[test]
    fn test_slice_target_parsing() {
        assert_eq!(parse_slice_target("foo:bar"), ("foo", Some("bar")));
        assert_eq!(
            parse_slice_target("src/main.rs::foo:bar"),
            ("src/main.rs::foo", Some("bar"))
        );
        assert_eq!(parse_slice_target("foo"), ("foo", None));
        assert_eq!(parse_slice_target("foo::bar"), ("foo::bar", None));
    }

    #[test]
    fn test_execute_slice_e2e() {
        let interner = Interner::new();
        let registry = LanguageRegistry::new().unwrap();

        let code = b"fn compute(x: i32) -> i32 {\n    let y = x + 1;\n    let z = y * 2;\n    return z;\n}\n";
        let ingest =
            ingest_in_memory_files(&[("src/lib.rs", Lang::Rust, code)], &registry, &interner);

        // List locals
        let locals_res = execute_slice(
            &ingest,
            &registry,
            &interner,
            "compute",
            SliceFlowDir::Fwd,
            8,
            Some(code),
        )
        .unwrap();
        assert!(locals_res.contains("<slice schema=\"ripwire.slice/v1\" sym=\"compute\""));
        assert!(locals_res.contains("<v n=\"x\""));
        assert!(locals_res.contains("<v n=\"y\""));

        // Slice x forward
        let fwd_res = execute_slice(
            &ingest,
            &registry,
            &interner,
            "compute:x",
            SliceFlowDir::Fwd,
            8,
            Some(code),
        )
        .unwrap();
        assert!(fwd_res.contains("flow=\"fwd\""));
        assert!(fwd_res.contains(
            "<s l=\"1\" k=\"param\" v=\"x\" d=\"0\"><![CDATA[fn compute(x: i32) -> i32 {]]></s>"
        ));
        assert!(fwd_res.contains("<s l=\"2\" k=\"read\" v=\"x\" d=\"1\" from=\"1\" rd=\"1\"><![CDATA[let y = x + 1;]]></s>"));
        assert!(fwd_res.contains("<s l=\"3\" k=\"read\" v=\"y\" d=\"2\" from=\"2\" rd=\"2\"><![CDATA[let z = y * 2;]]></s>"));
        assert!(fwd_res.contains(
            "<s l=\"4\" k=\"read\" v=\"z\" d=\"3\" from=\"3\" rd=\"3\"><![CDATA[return z;]]></s>"
        ));
    }
}
