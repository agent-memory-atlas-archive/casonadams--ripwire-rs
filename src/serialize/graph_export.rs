use crate::domain::{IngestResult, Interner, Symbol};
use crate::graph::Graph;
use crate::serialize::escape::escape_xml_write;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphFormat {
    GraphMl,
    NodeLinkJson,
}

impl GraphFormat {
    #[must_use]
    pub fn from_path(path: &str) -> Option<Self> {
        match Path::new(path).extension()?.to_str()? {
            "graphml" => Some(Self::GraphMl),
            "json" => Some(Self::NodeLinkJson),
            _ => None,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::GraphMl => "graphml",
            Self::NodeLinkJson => "node-link-json",
        }
    }
}

pub struct GraphExport<'a> {
    pub ingest: &'a IngestResult,
    pub graph: &'a Graph,
    pub ranks: &'a [f64],
    pub interner: &'a Interner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportCounts {
    pub nodes: usize,
    pub edges: usize,
}

impl GraphExport<'_> {
    pub fn write<W: Write>(&self, format: GraphFormat, out: &mut W) -> io::Result<ExportCounts> {
        match format {
            GraphFormat::GraphMl => self.write_graphml(out),
            GraphFormat::NodeLinkJson => self.write_node_link_json(out),
        }
    }

    fn node_fields(&self, sym: &Symbol) -> NodeFields<'_> {
        NodeFields {
            name: self.interner.resolve(&sym.name),
            kind: sym.kind.tag(),
            file: self.ingest.file_path(sym.file_id, self.interner),
            line: sym.line,
            lang: sym.lang.tag(),
            rank: self.ranks.get(sym.id as usize).copied().unwrap_or(0.0),
        }
    }

    fn write_graphml<W: Write>(&self, out: &mut W) -> io::Result<ExportCounts> {
        out.write_all(GRAPHML_HEADER.as_bytes())?;
        for sym in &self.ingest.symbols {
            let node = self.node_fields(sym);
            write!(out, "<node id=\"n{}\"><data key=\"name\">", sym.id)?;
            escape_xml_write(out, node.name)?;
            write!(
                out,
                "</data><data key=\"kind\">{}</data><data key=\"file\">",
                node.kind
            )?;
            escape_xml_write(out, node.file)?;
            writeln!(
                out,
                "</data><data key=\"line\">{}</data><data key=\"lang\">{}</data><data key=\"rank\">{}</data></node>",
                node.line, node.lang, node.rank
            )?;
        }
        let mut edges = 0;
        for sym in &self.ingest.symbols {
            for edge in self.graph.callee_edges(sym.id) {
                edges += 1;
                writeln!(
                    out,
                    "<edge source=\"n{}\" target=\"n{}\"><data key=\"prov\">{}</data><data key=\"type_use\">{}</data></edge>",
                    sym.id,
                    edge.to,
                    edge.prov.label(),
                    edge.weight == 0.0
                )?;
            }
        }
        out.write_all(b"</graph>\n</graphml>\n")?;
        Ok(ExportCounts {
            nodes: self.ingest.symbols.len(),
            edges,
        })
    }

    fn write_node_link_json<W: Write>(&self, out: &mut W) -> io::Result<ExportCounts> {
        out.write_all(b"{\"directed\":true,\"multigraph\":false,\"graph\":{},\"nodes\":[")?;
        for (i, sym) in self.ingest.symbols.iter().enumerate() {
            if i > 0 {
                out.write_all(b",\n")?;
            }
            let node = self.node_fields(sym);
            write!(out, "{{\"id\":{},\"name\":", sym.id)?;
            serde_json::to_writer(&mut *out, node.name)?;
            write!(out, ",\"kind\":\"{}\",\"file\":", node.kind)?;
            serde_json::to_writer(&mut *out, node.file)?;
            write!(
                out,
                ",\"line\":{},\"lang\":\"{}\",\"rank\":{}}}",
                node.line, node.lang, node.rank
            )?;
        }
        out.write_all(b"],\"links\":[")?;
        let mut edges = 0;
        for sym in &self.ingest.symbols {
            for edge in self.graph.callee_edges(sym.id) {
                if edges > 0 {
                    out.write_all(b",\n")?;
                }
                edges += 1;
                write!(
                    out,
                    "{{\"source\":{},\"target\":{},\"prov\":\"{}\",\"type_use\":{}}}",
                    sym.id,
                    edge.to,
                    edge.prov.label(),
                    edge.weight == 0.0
                )?;
            }
        }
        out.write_all(b"]}\n")?;
        Ok(ExportCounts {
            nodes: self.ingest.symbols.len(),
            edges,
        })
    }
}

struct NodeFields<'a> {
    name: &'a str,
    kind: &'static str,
    file: &'a str,
    line: u32,
    lang: &'static str,
    rank: f64,
}

const GRAPHML_HEADER: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">\n",
    "<key id=\"name\" for=\"node\" attr.name=\"name\" attr.type=\"string\"/>\n",
    "<key id=\"kind\" for=\"node\" attr.name=\"kind\" attr.type=\"string\"/>\n",
    "<key id=\"file\" for=\"node\" attr.name=\"file\" attr.type=\"string\"/>\n",
    "<key id=\"line\" for=\"node\" attr.name=\"line\" attr.type=\"int\"/>\n",
    "<key id=\"lang\" for=\"node\" attr.name=\"lang\" attr.type=\"string\"/>\n",
    "<key id=\"rank\" for=\"node\" attr.name=\"rank\" attr.type=\"double\"/>\n",
    "<key id=\"prov\" for=\"edge\" attr.name=\"prov\" attr.type=\"string\"/>\n",
    "<key id=\"type_use\" for=\"edge\" attr.name=\"type_use\" attr.type=\"boolean\"/>\n",
    "<graph id=\"G\" edgedefault=\"directed\">\n",
);
