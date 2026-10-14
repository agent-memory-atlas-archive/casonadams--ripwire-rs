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
        out.push_str(&format!(
            "  <v n=\"{name}\" l=\"{line}\" t=\"{}\" />\n",
            role.as_str()
        ));
    }
    out.push_str("</slice>\n");
    out
}

/// Render XML for transitive dataflow slice.
fn render_flow_xml(
    sym_name: &str,
    var_name: &str,
    path_line: &str,
    dir: SliceFlowDir,
    depth: u32,
    steps: &[SliceStep],
) -> String {
    let mut out = format!(
        "<slice schema=\"ripwire.slice/v1\" sym=\"{sym_name}\" var=\"{var_name}\" p=\"{path_line}\" flow=\"{}\" depth=\"{depth}\" steps=\"{}\" counts=\"as-classified\">\n",
        dir.as_str(),
        steps.len()
    );
    for s in steps {
        let role_str = OccRole::as_str(s.role);
        if s.from_line > 0 {
            out.push_str(&format!(
                "  <s l=\"{}\" k=\"{role_str}\" v=\"{}\" d=\"{}\" from=\"{}\" />\n",
                s.line, s.var_name, s.depth, s.from_line
            ));
        } else {
            out.push_str(&format!(
                "  <s l=\"{}\" k=\"{role_str}\" v=\"{}\" d=\"{}\" />\n",
                s.line, s.var_name, s.depth
            ));
        }
    }
    out.push_str("</slice>\n");
    out
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
    let (sym_query, var_opt) = parse_slice_target(target);

    let matches_query = |s: &&crate::domain::Symbol| {
        interner.resolve(&s.name) == sym_query
            || interner.resolve(&s.canonical_id) == sym_query
            || interner.resolve(&s.canonical_id).ends_with(sym_query)
    };
    let symbol = ingest
        .symbols
        .iter()
        .filter(matches_query)
        .find(|s| {
            s.kind == crate::domain::SymKind::Function || s.kind == crate::domain::SymKind::Method
        })
        .or_else(|| ingest.symbols.iter().find(matches_query))
        .ok_or_else(|| format!("symbol not found: {sym_query}"))?;
    if matches!(
        symbol.lang,
        crate::domain::Lang::Bash
            | crate::domain::Lang::Markdown
            | crate::domain::Lang::Yaml
            | crate::domain::Lang::Toml
            | crate::domain::Lang::Json
            | crate::domain::Lang::Html
            | crate::domain::Lang::Css
            | crate::domain::Lang::Sql
            | crate::domain::Lang::Proto
            | crate::domain::Lang::Graphql
            | crate::domain::Lang::Dockerfile
            | crate::domain::Lang::Hcl
    ) || symbol.kind == crate::domain::SymKind::Section
    {
        return Err(format!(
            "--slice: slice not served for {} yet — the def-use classification requires a verified per-grammar local variable AST",
            symbol.lang.tag()
        ));
    }
    let file_info = ingest
        .files
        .get(symbol.file_id as usize)
        .ok_or_else(|| "file info not found for symbol".to_string())?;

    let file_path = interner.resolve(&file_info.path);
    let source_bytes = match custom_source {
        Some(b) => b.to_vec(),
        None => {
            std::fs::read(file_path).map_err(|e| format!("cannot read file {file_path}: {e}"))?
        }
    };

    let (ts_lang, _) = registry
        .get(symbol.lang)
        .ok_or_else(|| format!("language not supported for slicing: {:?}", symbol.lang))?;

    let mut parser = Parser::new();
    parser
        .set_language(ts_lang)
        .map_err(|e| format!("failed to initialize parser: {e}"))?;

    let tree = parser
        .parse(&source_bytes, None)
        .ok_or_else(|| "failed to parse source file".to_string())?;

    let start_b = symbol.sig_start_byte as usize;
    let end_b = symbol.end_byte as usize;
    let sym_name = interner.resolve(&symbol.name);
    let path_line = format!("{file_path}:{}", symbol.line);

    match var_opt {
        None => {
            let locals = list_span_locals(tree.root_node(), start_b, end_b, &source_bytes);
            Ok(render_locals_xml(
                sym_name,
                &path_line,
                symbol.kind.tag(),
                symbol.lang.tag(),
                &locals,
            ))
        }
        Some(var_name) => {
            let all_occs = collect_span_variables(tree.root_node(), start_b, end_b, &source_bytes);
            let steps = compute_dataflow_slice(&all_occs, var_name, flow_dir, max_depth);
            Ok(render_flow_xml(
                sym_name, var_name, &path_line, flow_dir, max_depth, &steps,
            ))
        }
    }
}

/// Helper entry point for executing slicing from CLI arguments.
pub fn execute_slice_command(
    root: &str,
    target: Option<&str>,
    flow_str: &str,
) -> std::io::Result<()> {
    let Some(target) = target else {
        eprintln!("ripwire: target required (e.g. ripwire slice sym:var)");
        return Ok(());
    };

    let (ingest, _graph, _pr, interner) = crate::cli::load_repo_analysis(root)?;
    let registry = LanguageRegistry::new().map_err(|e| std::io::Error::other(e.to_string()))?;
    let (sym_query, _) = parse_slice_target(target);
    let matches_query = |s: &&crate::domain::Symbol| {
        interner.resolve(&s.name) == sym_query
            || interner.resolve(&s.canonical_id) == sym_query
            || interner.resolve(&s.canonical_id).ends_with(sym_query)
    };
    let custom_bytes = ingest
        .symbols
        .iter()
        .filter(matches_query)
        .find(|s| {
            s.kind == crate::domain::SymKind::Function || s.kind == crate::domain::SymKind::Method
        })
        .or_else(|| ingest.symbols.iter().find(matches_query))
        .and_then(|s| ingest.files.get(s.file_id as usize))
        .and_then(|fi| {
            let rel = interner.resolve(&fi.path);
            std::fs::read(std::path::Path::new(root).join(rel)).ok()
        });

    let flow_dir = SliceFlowDir::from_str_opt(flow_str).unwrap_or(SliceFlowDir::Fwd);
    match execute_slice(
        &ingest,
        &registry,
        &interner,
        target,
        flow_dir,
        8,
        custom_bytes.as_deref(),
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
        assert!(fwd_res.contains("<s l=\"1\" k=\"param\" v=\"x\""));
        assert!(fwd_res.contains("<s l=\"2\" k=\"read\" v=\"x\" d=\"1\" from=\"1\""));
        assert!(fwd_res.contains("<s l=\"3\" k=\"read\" v=\"y\" d=\"2\" from=\"2\""));
        assert!(fwd_res.contains("<s l=\"4\" k=\"read\" v=\"z\" d=\"3\" from=\"3\""));
    }
}
