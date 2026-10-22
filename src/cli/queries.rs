pub use super::arch::*;
pub use super::detail::*;
pub use super::retrieval::*;
pub use super::task_route::*;
pub use super::workspace::*;

use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::rank::PageRankResult;
use std::io::{self, Write};

pub fn execute_batch_content<W: Write>(
    root: &str,
    content: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    use crate::query::commands::{QueryContext, execute_batch, parse_batch_content};
    use crate::serialize::GitStampExt;
    let sources = crate::query::FileSources {
        root: std::path::Path::new(root),
    };
    let at = root.git_at_or_head();
    let context = QueryContext {
        root,
        ingest,
        graph,
        pr,
        interner,
        sources: &sources,
        commits: None,
        at: Some(&at),
    };
    let queries = parse_batch_content(content);
    let results = execute_batch(&context, &queries).map_err(io::Error::other)?;
    write_batch_results(root, &results, out)
}

pub fn write_batch_results<W: Write>(
    root: &str,
    results: &[serde_json::Value],
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlEscapeExt;
    writeln!(
        out,
        "<batch schema=\"ripwire-rs.batch/v1\" root=\"{}\" dictv=\"{}\" count=\"{}\">",
        root.xml_escaped(),
        crate::serialize::LEGEND_DICT_VERSION,
        results.len(),
    )?;
    for result in results {
        let name = result["tool"].as_str().unwrap_or("");
        writeln!(out, "  <result query=\"{}\">", name.xml_escaped())?;
        if let Some(error) = result.get("error").and_then(serde_json::Value::as_str) {
            writeln!(out, "<error>{}</error>", error.xml_escaped())?;
        } else if result["kind"] == "xml" {
            let value = result["result"]
                .as_str()
                .ok_or_else(|| io::Error::other("XML batch result must be a string"))?;
            out.write_all(value.as_bytes())?;
            writeln!(out)?;
        } else if let Some(value) = result["result"].as_str() {
            writeln!(out, "{}", value.xml_escaped())?;
        } else {
            let value = serde_json::to_string(&result["result"]).map_err(io::Error::other)?;
            writeln!(out, "{}", value.xml_escaped())?;
        }
        writeln!(out, "  </result>")?;
    }
    writeln!(out, "</batch>")
}
pub fn execute_batch_file<W: Write>(
    root: &str,
    file_path: &str,
    ingest: &IngestResult,
    graph: &Graph,
    pr: &PageRankResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let content = if file_path == "-" {
        let mut buf = String::new();
        io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
        buf
    } else {
        std::fs::read_to_string(file_path)?
    };
    execute_batch_content(root, &content, ingest, graph, pr, interner, out)
}
