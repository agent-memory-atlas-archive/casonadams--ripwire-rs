use serde_json::Value;
use std::io::{self, Write};

/// Serialize `for` task-lens ranking result to Ripwire XML.
pub fn serialize_for_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let task = val["task"].as_str().unwrap_or("");
    writeln!(
        out,
        "<ctx task=\"{task}\" schema=\"ripwire.for/v1\" root=\".\">"
    )?;
    if let Some(items) = val["symbols"].as_array() {
        for it in items {
            let name = it["name"].as_str().unwrap_or("");
            let path = it["path"].as_str().unwrap_or("");
            let line = it["line"].as_u64().unwrap_or(0);
            let score = it["score"].as_f64().unwrap_or(0.0);
            let kind = it["kind"].as_str().unwrap_or("fn");
            writeln!(
                out,
                "  <d id=\"{path}::{name}\" p=\"{path}\" l=\"{line}\" kind=\"{kind}\" score=\"{score:.2}\"><sig>{name}</sig></d>"
            )?;
        }
    }
    writeln!(out, "</ctx>")
}

/// Serialize `grep` search result to Ripwire XML.
pub fn serialize_grep_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let pat = val["pattern"].as_str().unwrap_or("");
    let count = val["count"].as_u64().unwrap_or(0);
    writeln!(
        out,
        "<grep pattern=\"{pat}\" schema=\"ripwire.grep/v1\" root=\".\" count=\"{count}\">"
    )?;
    if let Some(hits) = val["hits"].as_array() {
        for h in hits {
            let file = h["file"].as_str().unwrap_or("");
            let line = h["line"].as_u64().unwrap_or(0);
            let enc = h["enclosing"].as_str().unwrap_or("");
            let text = h["text"].as_str().unwrap_or("");
            writeln!(
                out,
                "  <f p=\"{file}\"><hit l=\"{line}\" in=\"{enc}\">{text}</hit></f>"
            )?;
        }
    }
    writeln!(out, "</grep>")
}

/// Serialize `whereis` branch symbol search to Ripwire XML.
pub fn serialize_whereis_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let sym = val["symbol"].as_str().unwrap_or("");
    let hits = val["hits"].as_array();
    let count = hits.map_or(0, |h| h.len());
    writeln!(
        out,
        "<whereis schema=\"ripwire.whereis/v1\" sym=\"{sym}\" root=\".\" count=\"{count}\">"
    )?;
    if let Some(items) = hits {
        for h in items {
            let path = h["path"].as_str().unwrap_or("");
            let line = h["line"].as_u64().unwrap_or(0);
            let kind = h["kind"].as_str().unwrap_or("");
            let text = h["text"].as_str().unwrap_or("");
            writeln!(
                out,
                "  <hit p=\"{path}\" l=\"{line}\" kind=\"{kind}\" t=\"{text}\"/>"
            )?;
        }
    }
    writeln!(out, "</whereis>")
}

/// Serialize `owners` authorship to Ripwire XML.
pub fn serialize_owners_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let files = val["files"].as_array();
    let count = files.map_or(0, |f| f.len());
    writeln!(
        out,
        "<owners schema=\"ripwire.owners/v1\" root=\".\" files=\"{count}\">"
    )?;
    if let Some(items) = files {
        for f in items {
            let path = f["path"].as_str().unwrap_or("");
            let authors = f["authors"].as_u64().unwrap_or(1);
            let top = f["top_author"].as_str().unwrap_or("");
            let share = f["top_share"].as_f64().unwrap_or(1.0);
            writeln!(
                out,
                "  <f p=\"{path}\" authors=\"{authors}\" top=\"{top}\" share=\"{share:.2}\"/>"
            )?;
        }
    }
    writeln!(out, "</owners>")
}

/// Serialize `flags` environment and compile gates to Ripwire XML.
pub fn serialize_flags_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let gates = val["gates"].as_array();
    let count = gates.map_or(0, |g| g.len());
    writeln!(
        out,
        "<flags schema=\"ripwire.flags/v1\" root=\".\" count=\"{count}\">"
    )?;
    if let Some(items) = gates {
        for g in items {
            let name = g["name"].as_str().unwrap_or("");
            let kind = g["kind"].as_str().unwrap_or("");
            let file = g["file"].as_str().unwrap_or("");
            let line = g["line"].as_u64().unwrap_or(0);
            writeln!(
                out,
                "  <gate name=\"{name}\" kind=\"{kind}\" p=\"{file}\" l=\"{line}\"/>"
            )?;
        }
    }
    writeln!(out, "</flags>")
}

/// Serialize `doc_drift` out-of-date document references to Ripwire XML.
pub fn serialize_doc_drift_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let drift = val["drift"].as_array();
    let count = drift.map_or(0, |d| d.len());
    writeln!(
        out,
        "<doc-drift schema=\"ripwire.doc-drift/v1\" root=\".\" drift=\"{count}\">"
    )?;
    if let Some(items) = drift {
        for d in items {
            let file = d["doc"].as_str().unwrap_or("");
            let line = d["line"].as_u64().unwrap_or(0);
            let ref_str = d["referenced"].as_str().unwrap_or("");
            writeln!(out, "  <a p=\"{file}\" l=\"{line}\" ref=\"{ref_str}\"/>")?;
        }
    }
    writeln!(out, "</doc-drift>")
}

/// Serialize `stray_content` branch divergence to Ripwire XML.
pub fn serialize_stray_content_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let branches = val["branches"].as_array();
    let count = branches.map_or(0, |b| b.len());
    writeln!(
        out,
        "<stray-content schema=\"ripwire.stray-content/v1\" branches=\"{count}\">"
    )?;
    if let Some(items) = branches {
        for b in items {
            let name = b["branch"].as_str().unwrap_or("");
            let unmerged = b["unmerged_commits"].as_u64().unwrap_or(0);
            writeln!(out, "  <ref name=\"{name}\" unmerged=\"{unmerged}\"/>")?;
        }
    }
    writeln!(out, "</stray-content>")
}

/// Serialize `mentions` doc backtick occurrences to Ripwire XML.
pub fn serialize_mentions_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let sym = val["symbol"].as_str().unwrap_or("");
    let docs = val["docs"].as_array();
    let count = docs.map_or(0, |d| d.len());
    writeln!(
        out,
        "<mentions schema=\"ripwire.mentions/v1\" of=\"{sym}\" root=\".\" docs=\"{count}\">"
    )?;
    if let Some(items) = docs {
        for d in items {
            let path = d["path"].as_str().unwrap_or("");
            let mentions = d["mentions"].as_u64().unwrap_or(0);
            writeln!(out, "  <doc p=\"{path}\" mentions=\"{mentions}\"/>")?;
        }
    }
    writeln!(out, "</mentions>")
}

/// Serialize `connect` subgraph to Ripwire XML.
pub fn serialize_connect_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let symbols = val["symbols"].as_array();
    let count = symbols.map_or(0, |s| s.len());
    writeln!(
        out,
        "<connect schema=\"ripwire.connect/v1\" root=\".\" symbols=\"{count}\">"
    )?;
    if let Some(edges) = val["edges"].as_array() {
        for e in edges {
            let from = e["from"].as_str().unwrap_or("");
            let to = e["to"].as_str().unwrap_or("");
            writeln!(out, "  <edge from=\"{from}\" to=\"{to}\"/>")?;
        }
    }
    writeln!(out, "</connect>")
}

/// Serialize `path_between` call path to Ripwire XML.
pub fn serialize_path_between_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let from = val["from"].as_str().unwrap_or("");
    let to = val["to"].as_str().unwrap_or("");
    let reachable = val["reachable"].as_bool().unwrap_or(false);
    let hops = val["hops"].as_u64().unwrap_or(0);
    writeln!(
        out,
        "<path-between schema=\"ripwire.path-between/v1\" from=\"{from}\" to=\"{to}\" reachable=\"{reachable}\" hops=\"{hops}\">"
    )?;
    if let Some(path) = val["path"].as_array() {
        for step in path {
            let s = step.as_str().unwrap_or("");
            writeln!(out, "  <step sym=\"{s}\"/>")?;
        }
    }
    writeln!(out, "</path-between>")
}
