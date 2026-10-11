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

fn find_node_at_span<'a>(root: Node<'a>, start_byte: usize, end_byte: usize) -> Node<'a> {
    let mut curr = root;
    loop {
        let mut found_child = None;
        for i in 0..curr.child_count() {
            if let Some(child) = curr.child(i)
                && child.start_byte() <= start_byte
                && child.end_byte() >= end_byte
            {
                found_child = Some(child);
                break;
            }
        }
        match found_child {
            Some(c) => curr = c,
            None => break curr,
        }
    }
}

/// Render XML for sliceable locals inventory when no variable was given.
fn render_locals_xml(sym_name: &str, path_line: &str, locals: &[(String, u32, OccRole)]) -> String {
    let mut out = format!(
        "<slice schema=\"ripwire.slice/v1\" sym=\"{sym_name}\" p=\"{path_line}\" counts=\"as-classified\">\n"
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
        if s.from_line > 0 {
            out.push_str(&format!(
                "  <s l=\"{}\" k=\"{}\" v=\"{}\" d=\"{}\" from=\"{}\" />\n",
                s.line,
                s.role.as_str(),
                s.var_name,
                s.depth,
                s.from_line
            ));
        } else {
            out.push_str(&format!(
                "  <s l=\"{}\" k=\"{}\" v=\"{}\" d=\"{}\" />\n",
                s.line,
                s.role.as_str(),
                s.var_name,
                s.depth
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

    let symbol = ingest
        .symbols
        .iter()
        .find(|s| {
            interner.resolve(&s.name) == sym_query
                || interner.resolve(&s.canonical_id) == sym_query
                || interner.resolve(&s.canonical_id).ends_with(sym_query)
        })
        .ok_or_else(|| format!("symbol not found: {sym_query}"))?;

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

    let def_node = find_node_at_span(
        tree.root_node(),
        symbol.sig_start_byte as usize,
        symbol.end_byte as usize,
    );

    let sym_name = interner.resolve(&symbol.name);
    let path_line = format!("{file_path}:{}", symbol.line);

    match var_opt {
        None => {
            let locals = list_sliceable_locals(def_node, &source_bytes);
            Ok(render_locals_xml(sym_name, &path_line, &locals))
        }
        Some(var_name) => {
            let all_occs = collect_symbol_variables(def_node, &source_bytes, None);
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

    let interner = Interner::new();
    let registry = LanguageRegistry::new().map_err(|e| std::io::Error::other(e.to_string()))?;

    let files = crate::crawler::crawl_directory(
        std::path::Path::new(root),
        &crate::crawler::CrawlerOptions::default(),
    );
    let ingest = crate::ingest::ingest_crawled_files(&files, &registry, &interner);

    let flow_dir = SliceFlowDir::from_str_opt(flow_str).unwrap_or(SliceFlowDir::Fwd);
    match execute_slice(&ingest, &registry, &interner, target, flow_dir, 8, None) {
        Ok(out) => {
            print!("{out}");
            Ok(())
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
