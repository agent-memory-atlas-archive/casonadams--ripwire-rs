use crate::serialize::escape::escape_xml_str;
use serde_json::Value;
use std::io::{self, Write};

fn write_xml_envelope<W: Write, F>(
    out: &mut W,
    tag: &str,
    header_attrs: &str,
    write_children: F,
) -> io::Result<()>
where
    F: FnOnce(&mut W) -> io::Result<()>,
{
    writeln!(out, "<{tag} {header_attrs}>")?;
    write_children(out)?;
    writeln!(out, "</{tag}>")
}

/// Serialize `for` task-lens ranking result to Ripwire XML.
pub fn serialize_for_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let task = escape_xml_str(val["task"].as_str().unwrap_or(""));
    let attrs = format!("task=\"{task}\" schema=\"ripwire.for/v1\" root=\".\"");
    write_xml_envelope(out, "ctx", &attrs, |out| {
        let mut bodies_to_show = Vec::new();
        if let Some(items) = val["symbols"].as_array() {
            for it in items {
                let name = escape_xml_str(it["name"].as_str().unwrap_or(""));
                let path = escape_xml_str(it["path"].as_str().unwrap_or(""));
                let line = it["line"].as_u64().unwrap_or(0);
                let score = it["score"].as_f64().unwrap_or(0.0);
                let kind = it["kind"].as_str().unwrap_or("fn");
                writeln!(
                    out,
                    "  <d id=\"{path}::{name}\" p=\"{path}\" l=\"{line}\" kind=\"{kind}\" score=\"{score:.2}\"><sig>{name}</sig></d>"
                )?;
                if let Some(b) = it.get("body").and_then(Value::as_str) {
                    bodies_to_show.push((kind, line, path, name, b));
                }
            }
        }
        if !bodies_to_show.is_empty() {
            let n = bodies_to_show.len();
            writeln!(out, "<bodies shown=\"{n}\" total=\"{n}\">")?;
            for (k, l, p, name, b) in bodies_to_show {
                writeln!(
                    out,
                    "<b t=\"{k}\" l=\"{l}\" p=\"{p}\" n=\"{name}\"><![CDATA[{b}]]></b>"
                )?;
            }
            writeln!(out, "</bodies>")?;
        }
        Ok(())
    })
}

/// Serialize `grep` search result to Ripwire XML.
pub fn serialize_grep_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let pat = escape_xml_str(val["pattern"].as_str().unwrap_or(""));
    let count = val["count"].as_u64().unwrap_or(0);
    let attrs =
        format!("pattern=\"{pat}\" schema=\"ripwire.grep/v1\" root=\".\" count=\"{count}\"");
    write_xml_envelope(out, "grep", &attrs, |out| {
        if let Some(hits) = val["hits"].as_array() {
            for h in hits {
                let file = escape_xml_str(h["file"].as_str().unwrap_or(""));
                let line = h["line"].as_u64().unwrap_or(0);
                let enc = escape_xml_str(h["enclosing"].as_str().unwrap_or(""));
                let text = escape_xml_str(h["text"].as_str().unwrap_or(""));
                writeln!(
                    out,
                    "  <f p=\"{file}\"><hit l=\"{line}\" in=\"{enc}\">{text}</hit></f>"
                )?;
            }
        }
        Ok(())
    })
}

/// Serialize `whereis` branch symbol search to Ripwire XML.
fn emit_whereis_fate<W: Write>(out: &mut W, fate: &Value, sym: &str) -> io::Result<()> {
    let v = fate.get("v").and_then(Value::as_str).unwrap_or("never");
    let mut row = format!("  <fate sym=\"{sym}\" v=\"{v}\"");
    if let Some(c) = fate.get("commit").and_then(Value::as_str) {
        row.push_str(&format!(" commit=\"{c}\""));
    }
    if let Some(d) = fate.get("date").and_then(Value::as_str) {
        row.push_str(&format!(" date=\"{d}\""));
    }
    if let Some(n) = fate.get("note").and_then(Value::as_str) {
        let n_esc = escape_xml_str(n);
        row.push_str(&format!(" note=\"{n_esc}\""));
    }
    row.push_str("/>");
    writeln!(out, "{row}")
}

fn emit_whereis_history<W: Write>(out: &mut W, hist: &Value, sym: &str) -> io::Result<()> {
    let head = hist.get("head").and_then(Value::as_str).unwrap_or("");
    writeln!(out, "  <history probed=\"1\" head=\"{head}\"/>")?;
    if let Some(fate) = hist.get("fate").and_then(Value::as_object) {
        let fate_val = Value::Object(fate.clone());
        emit_whereis_fate(out, &fate_val, sym)?;
    }
    Ok(())
}

fn emit_whereis_hit<W: Write>(out: &mut W, h: &Value) -> io::Result<()> {
    let ref_name = escape_xml_str(h.get("ref").and_then(Value::as_str).unwrap_or("HEAD"));
    let path = escape_xml_str(
        h.get("path")
            .or_else(|| h.get("file"))
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    let line = h["line"].as_u64().unwrap_or(0);
    let kind = h["kind"].as_str().unwrap_or("def");
    let text = escape_xml_str(h["text"].as_str().unwrap_or(""));
    writeln!(
        out,
        "  <hit ref=\"{ref_name}\" p=\"{path}\" l=\"{line}\" kind=\"{kind}\" t=\"{text}\"/>"
    )
}

/// Serialize `whereis` branch symbol search to Ripwire XML.
pub fn serialize_whereis_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let sym = escape_xml_str(val["symbol"].as_str().unwrap_or(""));
    let hits = val
        .get("hits")
        .or_else(|| val.get("locations"))
        .and_then(Value::as_array);
    let count = hits.map_or(0, |h| h.len());
    let on_head = val.get("on_head").and_then(Value::as_u64).unwrap_or(1);
    let refs_scanned = val.get("refs_scanned").and_then(Value::as_u64).unwrap_or(0);
    let blobs = val.get("blobs").and_then(Value::as_u64).unwrap_or(0);
    let at_attr = match val.get("at").and_then(Value::as_str) {
        Some(at) if !at.is_empty() => format!(" at=\"{at}\""),
        _ => String::new(),
    };
    let attrs = format!(
        "schema=\"ripwire.whereis/v1\" sym=\"{sym}\" on-head=\"{on_head}\" refs_scanned=\"{refs_scanned}\" blobs=\"{blobs}\" hits=\"{count}\" head_labels=\"index\"{at_attr}"
    );
    write_xml_envelope(out, "whereis", &attrs, |out| {
        if let Some(hist) = val.get("history").and_then(Value::as_object) {
            let hist_val = Value::Object(hist.clone());
            emit_whereis_history(out, &hist_val, &sym)?;
        }
        if let Some(items) = hits {
            for h in items {
                emit_whereis_hit(out, h)?;
            }
        }
        Ok(())
    })
}

/// Serialize `owners` authorship to Ripwire XML.
fn count_uniform_owners(files: &[Value]) -> usize {
    files
        .iter()
        .filter(|f| f.get("authors").and_then(Value::as_u64).unwrap_or(1) == 1)
        .count()
}

fn write_owner_entry<W: Write>(out: &mut W, f: &Value) -> io::Result<()> {
    let path = escape_xml_str(
        f.get("path")
            .or_else(|| f.get("file"))
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    let authors = f.get("authors").and_then(Value::as_u64).unwrap_or(1);
    let bf = f.get("bf").and_then(Value::as_u64).unwrap_or(1);
    let top = escape_xml_str(
        f.get("top_author")
            .or_else(|| f.get("top"))
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    let share = f
        .get("top_share")
        .or_else(|| f.get("share"))
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    writeln!(
        out,
        "  <f p=\"{path}\" authors=\"{authors}\" bf=\"{bf}\" top=\"{top}\" share=\"{share:.2}\"/>"
    )
}

fn emit_owner_items<W: Write>(out: &mut W, files: &[Value], detail: bool) -> io::Result<()> {
    for f in files {
        if !detail && f.get("authors").and_then(Value::as_u64).unwrap_or(1) == 1 {
            continue;
        }
        write_owner_entry(out, f)?;
    }
    Ok(())
}

/// Serialize `owners` authorship to Ripwire XML.
pub fn serialize_owners_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let files = val
        .get("files")
        .or_else(|| val.get("ownership"))
        .and_then(Value::as_array);
    let count = files.map_or(0, |f| f.len());
    let at_attr = match val.get("at").and_then(Value::as_str) {
        Some(at) if !at.is_empty() => format!(" at=\"{at}\""),
        _ => String::new(),
    };
    let detail = val.get("detail").and_then(Value::as_bool).unwrap_or(false);
    let uniform_count = files.map_or(0, |f| count_uniform_owners(f));

    let attrs = format!("schema=\"ripwire.owners/v1\" files=\"{count}\" root=\".\"{at_attr}");
    write_xml_envelope(out, "owners", &attrs, |out| {
        if !detail && uniform_count > 0 {
            writeln!(
                out,
                "  <uniform authors=\"1\" bf=\"1\" share=\"1.00\" files=\"{uniform_count}\"/>"
            )?;
        }
        if let Some(items) = files {
            emit_owner_items(out, items, detail)?;
        }
        Ok(())
    })
}

/// Serialize `flags` environment and compile gates to Ripwire XML.
pub fn serialize_flags_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let gates = val
        .get("gates")
        .or_else(|| val.get("flags"))
        .and_then(Value::as_array);
    let count = gates.map_or(0, |g| g.len());
    let files = val.get("files").and_then(Value::as_u64).unwrap_or(0);
    let compile = val
        .get("compile_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cmake = val.get("cmake_count").and_then(Value::as_u64).unwrap_or(0);
    let env = val.get("env_count").and_then(Value::as_u64).unwrap_or(0);
    let dark = val.get("dark_gates").and_then(Value::as_u64).unwrap_or(0);
    let attrs = format!(
        "schema=\"ripwire.flags/v1\" files=\"{files}\" gates=\"{count}\" dark_gates=\"{dark}\" compile=\"{compile}\" cmake=\"{cmake}\" env=\"{env}\""
    );
    write_xml_envelope(out, "flags", &attrs, |out| {
        if let Some(items) = gates {
            for g in items {
                let name = escape_xml_str(
                    g.get("name")
                        .or_else(|| g.get("flag"))
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                );
                let kind = g.get("kind").and_then(Value::as_str).unwrap_or("compile");
                let file = escape_xml_str(
                    g.get("file")
                        .or_else(|| g.get("path"))
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                );
                let line = g["line"].as_u64().unwrap_or(0);
                writeln!(
                    out,
                    "  <gate name=\"{name}\" kind=\"{kind}\" p=\"{file}\" l=\"{line}\"/>"
                )?;
            }
        }
        Ok(())
    })
}

/// Serialize `doc_drift` out-of-date document references to Ripwire XML.
pub fn serialize_doc_drift_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let drift = val["drift"].as_array();
    let count = drift.map_or(0, |d| d.len());
    let attrs = format!("schema=\"ripwire.doc-drift/v1\" root=\".\" drift=\"{count}\"");
    write_xml_envelope(out, "doc-drift", &attrs, |out| {
        if let Some(items) = drift {
            for d in items {
                let file = escape_xml_str(d["doc"].as_str().unwrap_or(""));
                let line = d["line"].as_u64().unwrap_or(0);
                let ref_str = escape_xml_str(d["referenced"].as_str().unwrap_or(""));
                writeln!(out, "  <a p=\"{file}\" l=\"{line}\" ref=\"{ref_str}\"/>")?;
            }
        }
        Ok(())
    })
}

/// Serialize `stray_content` branch divergence to Ripwire XML.
pub fn serialize_stray_content_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let head = escape_xml_str(val["head"].as_str().unwrap_or(""));
    let head_ref = escape_xml_str(val["head_ref"].as_str().unwrap_or("HEAD"));
    let refs = val["refs"].as_u64().unwrap_or(0);
    let blobs = val["blobs"].as_u64().unwrap_or(0);
    let unmerged = val["unmerged"].as_u64().unwrap_or(0);
    let superseded = val["superseded"].as_u64().unwrap_or(0);
    let merged = val["merged"].as_u64().unwrap_or(0);
    let unknown = val["unknown"].as_u64().unwrap_or(0);
    let at_attr = if let Some(at) = val.get("at").and_then(Value::as_str) {
        if !at.is_empty() {
            format!(" at=\"{at}\"")
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let attrs = format!(
        "schema=\"ripwire.stray-content/v1\" head=\"{head}\" head_ref=\"{head_ref}\" refs=\"{refs}\" blobs=\"{blobs}\" unmerged=\"{unmerged}\" superseded=\"{superseded}\" merged=\"{merged}\" unknown=\"{unknown}\"{at_attr}"
    );
    write_xml_envelope(out, "stray-content", &attrs, |out| {
        if let Some(branches) = val["branches"].as_array() {
            for b in branches {
                let name = escape_xml_str(
                    b["name"]
                        .as_str()
                        .or_else(|| b["branch"].as_str())
                        .unwrap_or(""),
                );
                let base = escape_xml_str(b["base"].as_str().unwrap_or(""));
                let verdict = escape_xml_str(b["verdict"].as_str().unwrap_or("unmerged"));
                let stray = b["stray"]
                    .as_u64()
                    .or_else(|| b["unmerged_commits"].as_u64())
                    .unwrap_or(0);
                writeln!(
                    out,
                    "  <ref name=\"{name}\" ok=\"1\" v=\"{verdict}\" base=\"{base}\" stray=\"{stray}\"/>"
                )?;
            }
        }
        Ok(())
    })
}

/// Serialize `mentions` doc backtick occurrences to Ripwire XML.
pub fn serialize_mentions_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let sym = escape_xml_str(val["symbol"].as_str().unwrap_or(""));
    let docs = val["docs"].as_array();
    let count = docs.map_or(0, |d| d.len());
    let attrs = format!("schema=\"ripwire.mentions/v1\" of=\"{sym}\" root=\".\" docs=\"{count}\"");
    write_xml_envelope(out, "mentions", &attrs, |out| {
        if let Some(items) = docs {
            for d in items {
                let path = escape_xml_str(
                    d.get("path")
                        .or_else(|| d.get("file"))
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                );
                let mentions = d["mentions"].as_u64().unwrap_or(0);
                writeln!(out, "  <doc p=\"{path}\" mentions=\"{mentions}\"/>")?;
            }
        }
        Ok(())
    })
}

/// Serialize `connect` subgraph to Ripwire XML.
pub fn serialize_connect_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let symbols = val["symbols"].as_array();
    let count = symbols.map_or(0, |s| s.len());
    let attrs = format!("schema=\"ripwire.connect/v1\" root=\".\" symbols=\"{count}\"");
    write_xml_envelope(out, "connect", &attrs, |out| {
        if let Some(edges) = val["edges"].as_array() {
            for e in edges {
                let from = escape_xml_str(e["from"].as_str().unwrap_or(""));
                let to = escape_xml_str(e["to"].as_str().unwrap_or(""));
                writeln!(out, "  <edge from=\"{from}\" to=\"{to}\"/>")?;
            }
        }
        Ok(())
    })
}

fn write_path_step<W: Write>(step: &Value, out: &mut W) -> io::Result<()> {
    let s = escape_xml_str(step.as_str().unwrap_or(""));
    writeln!(out, "  <s t=\"fn\" n=\"{s}\"/>")
}

/// Serialize `path_between` call path to Ripwire XML.
pub fn serialize_path_between_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let from = escape_xml_str(val["from"].as_str().unwrap_or(""));
    let to = escape_xml_str(val["to"].as_str().unwrap_or(""));
    let is_reach = val["reachable"].as_bool().unwrap_or(false);
    let reachable = if is_reach { "1" } else { "0" };
    let hops = val["hops"].as_u64().unwrap_or(0);
    write!(
        out,
        "<path schema=\"ripwire.path/v1\" from=\"{from}\" to=\"{to}\" reachable=\"{reachable}\" hops=\"{hops}\""
    )?;
    if !is_reach {
        write!(
            out,
            " hint=\"no directed call path — try --connect={from},{to} (undirected: finds a shared caller), or --uses/--impact for non-call references\""
        )?;
    }
    writeln!(out, ">")?;
    if let Some(path) = val["path"].as_array() {
        for step in path {
            write_path_step(step, out)?;
        }
    }
    writeln!(out, "</path>")
}
/// Serialize `--expand` symbol bodies to Ripwire XML.
pub fn serialize_expand_xml<W: Write>(
    root: &str,
    items: &[(&crate::domain::Symbol, &str, &str, &str)],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<ctx schema=\"ripwire.expand/v1\" root=\"{root}\"><bodies shown=\"{}\" total=\"{}\">",
        items.len(),
        items.len()
    )?;
    for (sym, name, path, body) in items {
        let kind = sym.kind.tag();
        let line = sym.line;
        let p_esc = escape_xml_str(path);
        let n_esc = escape_xml_str(name);
        writeln!(
            out,
            "  <b t=\"{kind}\" l=\"{line}\" p=\"{p_esc}\" n=\"{n_esc}\"><![CDATA[{body}]]></b>"
        )?;
    }
    writeln!(out, "</bodies></ctx>")
}

/// Top symbol representation in tree view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeFileSymbols<'a> {
    pub path: &'a str,
    pub total_symbols: usize,
    pub top_symbols: Vec<(&'a str, &'a str)>,
}

/// Serialize `--tree` hierarchical file symbol views to Ripwire XML.
pub fn serialize_tree_xml<W: Write>(
    root: &str,
    file_symbols: &[TreeFileSymbols<'_>],
    pr_iters: u32,
    out: &mut W,
) -> io::Result<()> {
    let total_shown: usize = file_symbols.iter().map(|f| f.top_symbols.len()).sum();
    write!(
        out,
        "<tree schema=\"ripwire.tree/v1\" files=\"{}\" pr_iters=\"{pr_iters}\" root=\"{root}\" shown_symbols=\"{total_shown}\">",
        file_symbols.len()
    )?;
    for f in file_symbols {
        let p_esc = escape_xml_str(f.path);
        write!(out, "<file p=\"{p_esc}\" symbols=\"{}\">", f.total_symbols)?;
        for &(kind, name) in &f.top_symbols {
            let n_esc = escape_xml_str(name);
            write!(out, "<s t=\"{kind}\" n=\"{n_esc}\"/>")?;
        }
        write!(out, "</file>")?;
    }
    writeln!(out, "</tree>")
}

/// Serialize `--pack-top-n` bodies to Ripwire XML.
pub fn serialize_pack_top_n_xml<W: Write>(
    root: &str,
    bodies: &[(&str, &str)],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<ctx schema=\"ripwire.pack-top-n/v1\" root=\"{root}\">"
    )?;
    for (path, body) in bodies {
        let p_esc = escape_xml_str(path);
        writeln!(out, "<src p=\"{p_esc}\">\n{body}\n</src>")?;
    }
    writeln!(out, "</ctx>")
}
/// Serialize `--test-gate` to Ripwire XML.
pub fn serialize_test_gate_xml<W: Write>(
    root: &str,
    tests: &[crate::gate::affected::AffectedTest],
    untested: usize,
    out: &mut W,
) -> io::Result<()> {
    let mut buf = format!(
        "<test-gate schema=\"ripwire.test-gate/v1\" root=\"{root}\" tests=\"{}\" untested=\"{untested}\">",
        tests.len()
    );
    for t in tests {
        let p_esc = escape_xml_str(&t.path);
        buf.push_str(&format!("<t p=\"{p_esc}\" hops=\"{}\"/>", t.hops));
    }
    buf.push_str("</test-gate>\n");
    out.write_all(buf.as_bytes())
}

/// Serialize `--safe-delete` to Ripwire XML.
pub fn serialize_safe_delete_xml<W: Write>(
    root: &str,
    sym_name: &str,
    callers: &[(&str, &str)],
    blast_radius: usize,
    out: &mut W,
) -> io::Result<()> {
    let risk = if callers.is_empty() && blast_radius == 0 {
        "none-found"
    } else {
        "uses-exist"
    };
    let sym_esc = escape_xml_str(sym_name);
    writeln!(
        out,
        "<safe-delete schema=\"ripwire.safe-delete/v1\" root=\"{root}\" sym=\"{sym_esc}\" callers=\"{}\" blast_radius=\"{blast_radius}\" risk=\"{risk}\">",
        callers.len()
    )?;
    for (p, n) in callers {
        let p_esc = escape_xml_str(p);
        let n_esc = escape_xml_str(n);
        writeln!(out, "  <caller p=\"{p_esc}\" n=\"{n_esc}\"/>")?;
    }
    writeln!(out, "</safe-delete>")
}

/// Serialize `--exercises` to Ripwire XML.
pub fn serialize_exercises_xml<W: Write>(
    root: &str,
    test_path: &str,
    reached: &[(&str, &str)],
    out: &mut W,
) -> io::Result<()> {
    let test_esc = escape_xml_str(test_path);
    writeln!(
        out,
        "<exercises schema=\"ripwire.exercises/v1\" root=\"{root}\" test=\"{test_esc}\" count=\"{}\">",
        reached.len()
    )?;
    for (p, n) in reached {
        let p_esc = escape_xml_str(p);
        let n_esc = escape_xml_str(n);
        writeln!(out, "  <target p=\"{p_esc}\" n=\"{n_esc}\"/>")?;
    }
    writeln!(out, "</exercises>")
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunityModule<'a> {
    pub id: u32,
    pub size: usize,
    pub dir: &'a str,
    pub label: &'a str,
    pub members: Vec<(&'static str, &'a str, &'a str, u32)>,
}

/// Serialize `--dead-code` candidates to Ripwire XML.
pub fn serialize_dead_code_xml<W: Write>(
    root: &str,
    items: &[(&str, &'static str, &str, u32)],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<dead-code schema=\"ripwire.dead-code/v1\" count=\"{}\" root=\"{root}\">",
        items.len()
    )?;
    for (n, t, p, l) in items {
        let n_esc = escape_xml_str(n);
        let p_esc = escape_xml_str(p);
        writeln!(
            out,
            "  <d n=\"{n_esc}\" t=\"{t}\" p=\"{p_esc}\" l=\"{l}\"/>"
        )?;
    }
    writeln!(out, "</dead-code>")
}

/// Serialize `--communities` to Ripwire XML.
pub fn serialize_communities_xml<W: Write>(
    root: &str,
    modules: &[CommunityModule<'_>],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<communities schema=\"ripwire.communities/v1\" root=\"{root}\" modules=\"{}\">",
        modules.len()
    )?;
    for m in modules {
        let dir_esc = escape_xml_str(m.dir);
        let lbl_esc = escape_xml_str(m.label);
        writeln!(
            out,
            "  <community id=\"{}\" size=\"{}\" dir=\"{dir_esc}\" label=\"{lbl_esc}\">",
            m.id, m.size
        )?;
        for &(kind, name, path, line) in &m.members {
            let n_esc = escape_xml_str(name);
            let p_esc = escape_xml_str(path);
            writeln!(
                out,
                "    <member t=\"{kind}\" n=\"{n_esc}\" p=\"{p_esc}:{line}\"/>"
            )?;
        }
        writeln!(out, "  </community>")?;
    }
    writeln!(out, "</communities>")
}

/// Serialize `--around` ego graph to Ripwire XML.
pub fn serialize_around_xml<W: Write>(
    root: &str,
    target: &str,
    depth: u32,
    fanout: usize,
    symbols: &[(&str, &'static str, &str, u32)],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<around schema=\"ripwire.around/v1\" root=\"{root}\" of=\"{target}\" depth=\"{depth}\" fanout=\"{fanout}\" count=\"{}\">",
        symbols.len()
    )?;
    for (name, kind, path, line) in symbols {
        writeln!(out, "  <s t=\"{kind}\" n=\"{name}\" p=\"{path}:{line}\"/>")?;
    }
    writeln!(out, "</around>")
}

/// Serialize `--external-surface` to Ripwire XML.
pub fn serialize_external_surface_xml<W: Write>(
    root: &str,
    externals: &[(&str, &str, usize)],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<external-surface schema=\"ripwire.external-surface/v1\" root=\"{root}\" names=\"{}\">",
        externals.len()
    )?;
    for (name, lang, refs) in externals {
        writeln!(out, "  <x n=\"{name}\" lang=\"{lang}\" refs=\"{refs}\"/>")?;
    }
    writeln!(out, "</external-surface>")
}

/// Serialize `--deps` to Ripwire XML.
pub fn serialize_deps_xml<W: Write>(
    root: &str,
    file_deps: &[(&str, usize, usize)],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<deps schema=\"ripwire.deps/v1\" root=\"{root}\" files=\"{}\">",
        file_deps.len()
    )?;
    for (p, inc, aff) in file_deps {
        writeln!(
            out,
            "  <f p=\"{p}\" includes=\"{inc}\" afferent=\"{aff}\"/>"
        )?;
    }
    writeln!(out, "</deps>")
}
/// Serialize `--exemplar` to Ripwire XML.
pub fn serialize_exemplar_xml<W: Write>(root: &str, val: &Value, out: &mut W) -> io::Result<()> {
    let ex = if val.get("exemplar").is_some() {
        &val["exemplar"]
    } else {
        val
    };
    let k = val
        .get("kind")
        .or_else(|| val.get("role"))
        .and_then(Value::as_str)
        .unwrap_or("fn");
    let n = ex.get("name").and_then(Value::as_str).unwrap_or_default();
    let p = ex.get("path").and_then(Value::as_str).unwrap_or_default();
    let l = ex.get("line").and_then(Value::as_u64).unwrap_or(1);
    let c = ex
        .get("callers")
        .or_else(|| ex.get("in"))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let cx = ex.get("ccx").and_then(Value::as_u64).unwrap_or_default();
    let cand_attr = match ex.get("candidates").and_then(Value::as_u64) {
        Some(cnt) => format!(" candidates=\"{cnt}\""),
        None => String::new(),
    };
    let tested = ex.get("tested").and_then(Value::as_u64).unwrap_or(0);
    let b = ex.get("body").and_then(Value::as_str).unwrap_or_default();

    writeln!(
        out,
        "<exemplar schema=\"ripwire.exemplar/v1\"{cand_attr} kind=\"{k}\" n=\"{n}\" p=\"{p}:{l}\" in=\"{c}\" ccx=\"{cx}\" root=\"{root}\" tested=\"{tested}\"><bodies shown=\"1\" total=\"1\"><b t=\"{k}\" l=\"{l}\" p=\"{p}\" n=\"{n}\"><![CDATA[{b}]]></b></bodies></exemplar>"
    )
}
/// Serialize `--lego` to Ripwire XML.
pub fn serialize_lego_xml<W: Write>(root: &str, val: &Value, out: &mut W) -> io::Result<()> {
    let iface = val["interface"].as_str().unwrap_or("");
    let empty = Vec::new();
    let impls = val["implementors"].as_array().unwrap_or(&empty);
    let mut buf = format!(
        "<ctx schema=\"ripwire.lego/v1\" root=\"{root}\"><lego><iface n=\"{iface}\" implementors=\"{}\">\n",
        impls.len()
    );
    for it in impls {
        let n = it["name"].as_str().unwrap_or("");
        let p = it["path"].as_str().unwrap_or("");
        buf.push_str(&format!("  <impl n=\"{n}\" p=\"{p}\"/>\n"));
    }
    buf.push_str("</iface></lego></ctx>\n");
    out.write_all(buf.as_bytes())
}

/// Serialize `--recall` to Ripwire XML.
pub fn serialize_recall_xml<W: Write>(
    root: &str,
    query: &str,
    val: &Value,
    out: &mut W,
) -> io::Result<()> {
    let empty = Vec::new();
    let matches = val["matches"].as_array().unwrap_or(&empty);
    let mut buf = format!(
        "<recall schema=\"ripwire.recall/v1\" root=\"{root}\" query=\"{query}\" docs=\"{}\">\n",
        matches.len()
    );
    for d in matches {
        let p = d["path"].as_str().unwrap_or("");
        let text = d["snippet"].as_str().unwrap_or("");
        buf.push_str(&format!("  <doc p=\"{p}\"><![CDATA[{text}]]></doc>\n"));
    }
    buf.push_str("</recall>\n");
    out.write_all(buf.as_bytes())
}

/// Serialize `--from-trace` to Ripwire XML.
pub fn serialize_from_trace_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let empty = Vec::new();
    let suspects = val["suspects"].as_array().unwrap_or(&empty);
    let mut buf = format!(
        "<from-trace schema=\"ripwire.from-trace/v1\" count=\"{}\">\n",
        suspects.len()
    );
    for s in suspects {
        let n = s["name"].as_str().unwrap_or("");
        let p = s["path"].as_str().unwrap_or("");
        let l = s["line"].as_u64().unwrap_or(0);
        buf.push_str(&format!("  <suspect n=\"{n}\" p=\"{p}\" l=\"{l}\"/>\n"));
    }
    buf.push_str("</from-trace>\n");
    out.write_all(buf.as_bytes())
}
