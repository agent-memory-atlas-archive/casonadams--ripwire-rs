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
    let task = val["task"].as_str().unwrap_or("");
    let attrs = format!("task=\"{task}\" schema=\"ripwire.for/v1\" root=\".\"");
    write_xml_envelope(out, "ctx", &attrs, |out| {
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
        Ok(())
    })
}

/// Serialize `grep` search result to Ripwire XML.
pub fn serialize_grep_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let pat = val["pattern"].as_str().unwrap_or("");
    let count = val["count"].as_u64().unwrap_or(0);
    let attrs =
        format!("pattern=\"{pat}\" schema=\"ripwire.grep/v1\" root=\".\" count=\"{count}\"");
    write_xml_envelope(out, "grep", &attrs, |out| {
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
        Ok(())
    })
}

/// Serialize `whereis` branch symbol search to Ripwire XML.
pub fn serialize_whereis_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let sym = val["symbol"].as_str().unwrap_or("");
    let hits = val["hits"].as_array();
    let count = hits.map_or(0, |h| h.len());
    let attrs = format!("schema=\"ripwire.whereis/v1\" sym=\"{sym}\" root=\".\" count=\"{count}\"");
    write_xml_envelope(out, "whereis", &attrs, |out| {
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
        Ok(())
    })
}

/// Serialize `owners` authorship to Ripwire XML.
pub fn serialize_owners_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let files = val["files"].as_array();
    let count = files.map_or(0, |f| f.len());
    let attrs = format!("schema=\"ripwire.owners/v1\" root=\".\" files=\"{count}\"");
    write_xml_envelope(out, "owners", &attrs, |out| {
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
        Ok(())
    })
}

/// Serialize `flags` environment and compile gates to Ripwire XML.
pub fn serialize_flags_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let gates = val["gates"].as_array();
    let count = gates.map_or(0, |g| g.len());
    let attrs = format!("schema=\"ripwire.flags/v1\" root=\".\" count=\"{count}\"");
    write_xml_envelope(out, "flags", &attrs, |out| {
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
                let file = d["doc"].as_str().unwrap_or("");
                let line = d["line"].as_u64().unwrap_or(0);
                let ref_str = d["referenced"].as_str().unwrap_or("");
                writeln!(out, "  <a p=\"{file}\" l=\"{line}\" ref=\"{ref_str}\"/>")?;
            }
        }
        Ok(())
    })
}

/// Serialize `stray_content` branch divergence to Ripwire XML.
pub fn serialize_stray_content_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let branches = val["branches"].as_array();
    let count = branches.map_or(0, |b| b.len());
    let attrs = format!("schema=\"ripwire.stray-content/v1\" branches=\"{count}\"");
    write_xml_envelope(out, "stray-content", &attrs, |out| {
        if let Some(items) = branches {
            for b in items {
                let name = b["branch"].as_str().unwrap_or("");
                let unmerged = b["unmerged_commits"].as_u64().unwrap_or(0);
                writeln!(out, "  <ref name=\"{name}\" unmerged=\"{unmerged}\"/>")?;
            }
        }
        Ok(())
    })
}

/// Serialize `mentions` doc backtick occurrences to Ripwire XML.
pub fn serialize_mentions_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let sym = val["symbol"].as_str().unwrap_or("");
    let docs = val["docs"].as_array();
    let count = docs.map_or(0, |d| d.len());
    let attrs = format!("schema=\"ripwire.mentions/v1\" of=\"{sym}\" root=\".\" docs=\"{count}\"");
    write_xml_envelope(out, "mentions", &attrs, |out| {
        if let Some(items) = docs {
            for d in items {
                let path = d["path"].as_str().unwrap_or("");
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
                let from = e["from"].as_str().unwrap_or("");
                let to = e["to"].as_str().unwrap_or("");
                writeln!(out, "  <edge from=\"{from}\" to=\"{to}\"/>")?;
            }
        }
        Ok(())
    })
}

fn write_path_step<W: Write>(step: &Value, out: &mut W) -> io::Result<()> {
    let s = step.as_str().unwrap_or("");
    writeln!(out, "  <s t=\"fn\" n=\"{s}\"/>")
}

/// Serialize `path_between` call path to Ripwire XML.
pub fn serialize_path_between_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let from = val["from"].as_str().unwrap_or("");
    let to = val["to"].as_str().unwrap_or("");
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
        writeln!(
            out,
            "  <b t=\"{kind}\" l=\"{line}\" p=\"{path}\" n=\"{name}\"><![CDATA[{body}]]></b>"
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
        write!(
            out,
            "<file p=\"{}\" symbols=\"{}\">",
            f.path, f.total_symbols
        )?;
        for &(kind, name) in &f.top_symbols {
            write!(out, "<s t=\"{kind}\" n=\"{name}\"/>")?;
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
        writeln!(out, "<src p=\"{path}\">\n{body}\n</src>")?;
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
        buf.push_str(&format!("<t p=\"{}\" hops=\"{}\"/>", t.path, t.hops));
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
    writeln!(
        out,
        "<safe-delete schema=\"ripwire.safe-delete/v1\" root=\"{root}\" sym=\"{sym_name}\" callers=\"{}\" blast_radius=\"{blast_radius}\" risk=\"{risk}\">",
        callers.len()
    )?;
    for (p, n) in callers {
        writeln!(out, "  <caller p=\"{p}\" n=\"{n}\"/>")?;
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
    writeln!(
        out,
        "<exercises schema=\"ripwire.exercises/v1\" root=\"{root}\" test=\"{test_path}\" count=\"{}\">",
        reached.len()
    )?;
    for (p, n) in reached {
        writeln!(out, "  <target p=\"{p}\" n=\"{n}\"/>")?;
    }
    writeln!(out, "</exercises>")
}
/// Community module representation.
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
        writeln!(out, "  <d n=\"{n}\" t=\"{t}\" p=\"{p}\" l=\"{l}\"/>")?;
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
        writeln!(
            out,
            "  <community id=\"{}\" size=\"{}\" dir=\"{}\" label=\"{}\">",
            m.id, m.size, m.dir, m.label
        )?;
        for &(kind, name, path, line) in &m.members {
            writeln!(
                out,
                "    <member t=\"{kind}\" n=\"{name}\" p=\"{path}:{line}\"/>"
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
    let ex = &val["exemplar"];
    let k = val.get("kind").and_then(Value::as_str).unwrap_or("fn");
    let n = ex.get("name").and_then(Value::as_str).unwrap_or_default();
    let p = ex.get("path").and_then(Value::as_str).unwrap_or_default();
    let l = ex.get("line").and_then(Value::as_u64).unwrap_or(1);
    let c = ex
        .get("callers")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let cx = ex.get("ccx").and_then(Value::as_u64).unwrap_or_default();
    let b = ex.get("body").and_then(Value::as_str).unwrap_or_default();

    writeln!(
        out,
        "<exemplar schema=\"ripwire.exemplar/v1\" root=\"{root}\" kind=\"{k}\" n=\"{n}\" p=\"{p}:{l}\" in=\"{c}\" ccx=\"{cx}\"><bodies shown=\"1\" total=\"1\"><b t=\"{k}\" l=\"{l}\" p=\"{p}\" n=\"{n}\"><![CDATA[{b}]]></b></bodies></exemplar>"
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
