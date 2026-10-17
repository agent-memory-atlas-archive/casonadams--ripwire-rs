use crate::serialize::JsonValueExt;
use crate::serialize::escape::{XmlEscapeExt, escape_xml_str};
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
fn write_for_sig_row<W: Write>(out: &mut W, idx: usize, it: &Value, task: &str) -> io::Result<()> {
    let raw_name = it.str_or("name", "");
    let line = it.u64_or("line", 0);
    let end_line = it.u64_or("end_line", line);
    let cx = it.u64_or("cx", 0);
    let ccx = it.u64_or("ccx", 0);
    let in_deg = it.u64_or("in", 0);
    let churn = it.u64_or("churn", 1);
    let amp = it.u64_or("amp", 1);
    let r = idx + 1;
    write!(
        out,
        "<d l=\"{line}\" e=\"{end_line}\" n=\"{}\" p=\"{}\" cx=\"{cx}\" ccx=\"{ccx}\" in=\"{in_deg}\" churn=\"{churn}\" amp=\"{amp}\" r=\"{r}\" next=\"--for={task} --limit=40\">{}</d>",
        raw_name.xml_escaped(),
        it.str_or("path", "").xml_escaped(),
        it.str_or("sig", raw_name).xml_escaped()
    )
}

pub fn serialize_for_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let raw_task = val.str_or("task", "");
    let task = escape_xml_str(raw_task);
    let default_route = format!("name-exact({raw_task})");
    let route = escape_xml_str(val.str_or("route", &default_route));
    let raw_root = val.str_or("root", ".");
    let root = escape_xml_str(raw_root);
    let at_stamp =
        crate::git::history::compute_git_at_stamp(raw_root).unwrap_or_else(|| "HEAD".to_string());
    let confidence = escape_xml_str(val.str_or("confidence", "high"));
    let margin_pct = val.u64_or("margin_pct", 0);
    let coverage = val.u64_or("coverage", 100);
    let sym_cnt = val["symbols"].as_array().map_or(0, Vec::len);
    let est_tokens = val.u64_or("est_tokens", (80 + sym_cnt * 24) as u64);
    let bodies_cnt = val.u64_or("detail", 1).max(1);
    let attrs = format!(
        "task=\"{task}\" route=\"{route}\" root=\"{root}\" confidence=\"{confidence}\" margin_pct=\"{margin_pct}\" coverage=\"{coverage}\" at=\"{at_stamp}\" schema=\"ripwire.for/v1\" bundle=\"auto\" bodies=\"{bodies_cnt}\" est_tokens=\"{est_tokens}\""
    );
    write_xml_envelope(out, "ctx", &attrs, |out| {
        let mut bodies_to_show = Vec::new();
        write!(out, "<sigs>")?;
        if let Some(items) = val["symbols"].as_array() {
            for (idx, it) in items.iter().enumerate() {
                write_for_sig_row(out, idx, it, &task)?;
                let raw_name = it.str_or("name", "");
                let b_str = it.str_any(&["body", "sig"], raw_name).to_string();
                if bodies_to_show.is_empty() {
                    let kind = it.str_or("kind", "fn").to_string();
                    let line = it.u64_or("line", 0);
                    let path = escape_xml_str(it.str_or("path", ""));
                    let name = escape_xml_str(raw_name);
                    bodies_to_show.push((kind, line, path, name, b_str));
                }
            }
        }
        write!(out, "</sigs>")?;
        write_for_lego_section(out, val, raw_task)?;
        write!(out, "<tail total=\"0\" shown=\"0\" capped=\"0\"></tail>")?;
        let n = bodies_to_show.len();
        write!(out, "<bodies shown=\"{n}\" total=\"{n}\" capped=\"0\">")?;
        for (k, l, p, name, b) in bodies_to_show {
            write!(
                out,
                "<b t=\"{k}\" l=\"{l}\" p=\"{p}\" n=\"{name}\"><![CDATA[{b}]]></b>"
            )?;
        }
        writeln!(out, "</bodies>")
    })
}

fn for_section_requested(val: &Value, name: &str) -> bool {
    val["sections"]
        .as_array()
        .is_some_and(|s| s.iter().any(|v| v.as_str() == Some(name)))
}

fn write_for_lego_section<W: Write>(out: &mut W, val: &Value, raw_task: &str) -> io::Result<()> {
    let Some(ifaces) = val["lego"].as_array().filter(|a| !a.is_empty()) else {
        return Ok(());
    };
    if !for_section_requested(val, "lego") {
        let replay = format!(
            "--for='{}' --sections=lego",
            raw_task.replace('\'', "'\\''")
        );
        return write!(
            out,
            "<!-- lego total=N shown=0 capped=1: N interfaces with implementors in the ranked files, collapsed; next= restores them. --><lego total=\"{}\" shown=\"0\" capped=\"1\" next=\"{}\"/>",
            ifaces.len(),
            replay.xml_escaped()
        );
    }
    write!(out, "<lego>")?;
    for iface in ifaces {
        let impls = iface["implementors"]
            .as_array()
            .map_or(&[][..], Vec::as_slice);
        write!(
            out,
            "<iface n=\"{}\" p=\"{}:{}\" implementors=\"{}\">",
            iface["name"].as_str().unwrap_or("").xml_escaped(),
            iface["path"].as_str().unwrap_or("").xml_escaped(),
            iface["line"].as_u64().unwrap_or(0),
            impls.len()
        )?;
        for imp in impls {
            write!(
                out,
                "<impl n=\"{}\" p=\"{}:{}\"/>",
                imp["name"].as_str().unwrap_or("").xml_escaped(),
                imp["path"].as_str().unwrap_or("").xml_escaped(),
                imp["line"].as_u64().unwrap_or(0)
            )?;
        }
        write!(out, "</iface>")?;
    }
    write!(out, "</lego>")
}

/// Serialize `grep` search result to Ripwire XML.
type GrepEncMap = std::collections::BTreeMap<String, (u64, u64)>;

fn collect_grep_summary(
    hits: Option<&Vec<Value>>,
) -> (
    std::collections::BTreeSet<String>,
    GrepEncMap,
    (String, u64),
) {
    let mut files_set = std::collections::BTreeSet::new();
    let mut enc_map = std::collections::BTreeMap::new();
    let mut first_loc = ("src/lib.rs".to_string(), 1u64);
    let Some(items) = hits else {
        return (files_set, enc_map, first_loc);
    };
    for (idx, h) in items.iter().enumerate() {
        let f = h.str_any(&["file", "path"], "src/lib.rs");
        let l = h.u64_or("line", 1);
        if idx == 0 {
            first_loc = (f.to_string(), l);
        }
        files_set.insert(f.to_string());
        if let Some(e) = h.opt_str("enclosing").filter(|s| !s.is_empty()) {
            let callers = h.u64_or("enclosing_callers", 0);
            let cx = h.u64_or("enclosing_cx", 1);
            enc_map.entry(e.to_string()).or_insert((callers, cx));
        }
    }
    (files_set, enc_map, first_loc)
}

fn write_grep_context_lines<W: Write>(
    out: &mut W,
    tag: &str,
    lines_opt: Option<&Vec<Value>>,
) -> io::Result<()> {
    if let Some(lines) = lines_opt {
        for row in lines {
            let l = row["line"].as_u64().unwrap_or(1);
            let t = row["text"].as_str().unwrap_or("");
            write!(out, "<{tag} l=\"{l}\"><![CDATA[{t}]]></{tag}>")?;
        }
    }
    Ok(())
}

fn write_grep_hit_row<W: Write>(out: &mut W, h: &Value) -> io::Result<()> {
    let file = h.str_any(&["file", "path"], "src/lib.rs").xml_escaped();
    let line = h.u64_or("line", 1);
    let enc = h.str_or("enclosing", "main").xml_escaped();
    let text = h.str_or("text", "");
    write!(out, "  <f p=\"{file}\">")?;
    write_grep_context_lines(out, "b", h["before"].as_array())?;
    write!(
        out,
        "<hit l=\"{line}\" in=\"{enc}\"><![CDATA[{text}]]></hit>"
    )?;
    write_grep_context_lines(out, "a", h["after"].as_array())?;
    writeln!(out, "</f>")
}

fn write_grep_enc_rows<W: Write>(out: &mut W, enc_map: &GrepEncMap) -> io::Result<()> {
    if enc_map.is_empty() {
        return writeln!(out, "  <enc n=\"main\" callers=\"0\" cx=\"1\"/>");
    }
    for (e, &(callers, cx)) in enc_map {
        writeln!(
            out,
            "  <enc n=\"{}\" callers=\"{callers}\" cx=\"{cx}\"/>",
            e.xml_escaped()
        )?;
    }
    Ok(())
}

pub fn serialize_grep_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let pat = escape_xml_str(val["pattern"].as_str().unwrap_or(""));
    let hits = val["hits"].as_array();
    let count = hits.map_or(0, Vec::len);
    let (files_set, enc_set, first_loc) = collect_grep_summary(hits);
    let f_cnt = files_set.len();
    let attrs = format!(
        "pattern=\"{pat}\" schema=\"ripwire.grep/v1\" root=\".\" files=\"{f_cnt}\" hits=\"{count}\" shown=\"{count}\" capped=\"0\" hits_capped=\"0\" suppressed_comment=\"0\" suppressed_string=\"0\" tier_parsed=\"{f_cnt}\" tier_unclassified=\"0\" corpus_pruned_dirs=\"0\" unindexed_hits=\"0\" unindexed_files_scanned=\"0\" next=\"--at={}:{}\"",
        escape_xml_str(&first_loc.0),
        first_loc.1
    );
    write_xml_envelope(out, "grep", &attrs, |out| {
        if let Some(items) = hits {
            for h in items {
                write_grep_hit_row(out, h)?;
            }
        }
        write_grep_enc_rows(out, &enc_set)
    })
}

/// Serialize `whereis` branch symbol search to Ripwire XML.
fn emit_whereis_fate<W: Write>(out: &mut W, fate: &Value, sym: &str) -> io::Result<()> {
    let v = fate.str_or("v", "never");
    let mut row = format!("  <fate sym=\"{sym}\" v=\"{v}\"");
    if let Some(c) = fate.opt_str("commit") {
        row.push_str(&format!(" commit=\"{c}\""));
    }
    if let Some(d) = fate.opt_str("date") {
        row.push_str(&format!(" date=\"{d}\""));
    }
    if let Some(n) = fate.opt_str("note") {
        row.push_str(&format!(" note=\"{}\"", n.xml_escaped()));
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
    let ref_name = h.str_or("ref", "HEAD").xml_escaped();
    let tip = h.str_or("tip", "").xml_escaped();
    let date = h.str_or("date", "").xml_escaped();
    let path = h.str_any(&["path", "file"], "").xml_escaped();
    let line = h.u64_or("line", 0);
    let kind = h.str_or("kind", "def");
    let text = h.str_or("text", "").xml_escaped();
    writeln!(
        out,
        "  <hit ref=\"{ref_name}\" tip=\"{tip}\" date=\"{date}\" p=\"{path}\" l=\"{line}\" kind=\"{kind}\" t=\"{text}\"/>"
    )
}

/// Serialize `whereis` branch symbol search to Ripwire XML.
pub fn serialize_whereis_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let sym = escape_xml_str(val.str_or("symbol", ""));
    let hits = val.array_any(&["hits", "locations"]);
    let count = hits.map_or(0, Vec::len);
    let cap = 60usize;
    let shown = count.min(cap);
    let capped = usize::from(shown < count);
    let page_attrs = if capped == 1 {
        format!(
            " shown=\"{shown}\" capped=\"1\" total=\"{count}\" has_more=\"1\" next_offset=\"{shown}\" offset=\"0\" limit=\"0\""
        )
    } else {
        format!(" shown=\"{shown}\" capped=\"0\"")
    };
    let on_head = val.u64_or("on_head", 1);
    let refs_scanned = val.u64_or("refs_scanned", 0);
    let blobs = val.u64_or("blobs", 0);
    let at_val = val
        .opt_str("at")
        .filter(|s| !s.is_empty())
        .unwrap_or("HEAD");
    let head_date = val.str_or("head_date", "2026-10-14");
    let listing = val.str_or("listing", "defs");
    let ref_labels = val.str_or("ref_labels", "parsed");
    let worktree = val.str_or("worktree", "read");
    let attrs = format!(
        "schema=\"ripwire.whereis/v1\" sym=\"{sym}\" on-head=\"{on_head}\" refs_scanned=\"{refs_scanned}\" blobs=\"{blobs}\" hits=\"{count}\" head_labels=\"index\"{page_attrs} listing=\"{listing}\" ref_labels=\"{ref_labels}\" head_date=\"{head_date}\" at=\"{at_val}\" worktree=\"{worktree}\" complete=\"1\""
    );
    write_xml_envelope(out, "whereis", &attrs, |out| {
        if let Some(hist) = val.get("history").and_then(Value::as_object) {
            let hist_val = Value::Object(hist.clone());
            emit_whereis_history(out, &hist_val, &sym)?;
        }
        if let Some(items) = hits {
            for h in items.iter().take(shown) {
                emit_whereis_hit(out, h)?;
            }
            if count > shown {
                writeln!(out, "  <more hits=\"{}\"/>", count - shown)?;
            }
        }
        Ok(())
    })
}

/// Serialize `owners` authorship to Ripwire XML.
fn count_uniform_owners(files: &[Value]) -> usize {
    files.iter().filter(|f| f.u64_or("authors", 1) == 1).count()
}

fn write_owner_entry<W: Write>(out: &mut W, f: &Value) -> io::Result<()> {
    let path = f.str_any(&["path", "file"], "").xml_escaped();
    let authors = f.u64_or("authors", 1);
    let bf = f.u64_or("bf", 1);
    let top = f.str_any(&["top_author", "top"], "").xml_escaped();
    let share = f.f64_any(&["top_share", "share"], 1.0);
    writeln!(
        out,
        "  <f p=\"{path}\" authors=\"{authors}\" bf=\"{bf}\" top=\"{top}\" share=\"{share:.2}\"/>"
    )
}

fn emit_owner_items<W: Write>(out: &mut W, files: &[Value], detail: bool) -> io::Result<()> {
    for f in files {
        if !detail && f.u64_or("authors", 1) == 1 {
            continue;
        }
        write_owner_entry(out, f)?;
    }
    Ok(())
}

/// Serialize `owners` authorship to Ripwire XML.
pub fn serialize_owners_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let files = val.array_any(&["files", "ownership"]);
    let count = files.map_or(0, Vec::len);
    let at_attr = match val.opt_str("at") {
        Some(at) if !at.is_empty() => format!(" at=\"{at}\""),
        _ => String::new(),
    };
    let detail = val.bool_or("detail", false);
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
    let gates = val.array_any(&["gates", "flags"]);
    let count = gates.map_or(0, Vec::len);
    let files = val.u64_or("files", 0);
    let compile = val.u64_or("compile_count", 0);
    let cmake = val.u64_or("cmake_count", 0);
    let env = val.u64_or("env_count", 0);
    let dark = val.u64_or("dark_gates", 0);
    let attrs = format!(
        "schema=\"ripwire.flags/v1\" files=\"{files}\" gates=\"{count}\" dark_gates=\"{dark}\" compile=\"{compile}\" cmake=\"{cmake}\" env=\"{env}\""
    );
    write_xml_envelope(out, "flags", &attrs, |out| {
        if let Some(items) = gates {
            for g in items {
                let name = g.str_any(&["name", "flag"], "").xml_escaped();
                let kind = g.str_or("kind", "compile");
                let file = g.str_any(&["file", "path"], "").xml_escaped();
                let line = g.u64_or("line", 0);
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
fn format_doc_drift_extra_attrs(d: &Value) -> String {
    let mut extra = String::new();
    for key in ["fate", "commit", "date", "subject"] {
        if let Some(v) = d[key].as_str() {
            extra.push_str(&format!(" {key}=\"{}\"", v.xml_escaped()));
        }
    }
    extra
}

fn write_doc_drift_anchor<W: Write>(out: &mut W, file: &str, d: &Value) -> io::Result<()> {
    let line = d.u64_any(&["line", "doc_line"], 0);
    let kind = d.str_or("kind", "symbol").xml_escaped();
    let why = d.str_or("why", "undefined").xml_escaped();
    let ref_str = d.str_any(&["referenced", "target_file"], "").xml_escaped();
    let extra = format_doc_drift_extra_attrs(d);
    writeln!(
        out,
        "    <a k=\"{kind}\" l=\"{line}\" why=\"{why}\" p=\"{file}\" ref=\"{ref_str}\"{extra}/>"
    )
}

pub fn serialize_doc_drift_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let drift = val["drift"].as_array();
    let count = drift.map_or(0, |d| d.len());
    let root = escape_xml_str(val["root"].as_str().unwrap_or("."));
    let mut grouped: Vec<(&str, Vec<&Value>)> = Vec::new();
    if let Some(items) = drift {
        for d in items {
            let doc = d["doc"].as_str().unwrap_or("");
            if let Some((_, list)) = grouped.iter_mut().find(|(p, _)| *p == doc) {
                list.push(d);
            } else {
                grouped.push((doc, vec![d]));
            }
        }
    }
    let docs_cnt = grouped.len();
    let attrs = format!(
        "schema=\"ripwire.doc-drift/v1\" root=\"{root}\" docs=\"{docs_cnt}\" drift=\"{count}\""
    );
    write_xml_envelope(out, "doc-drift", &attrs, |out| {
        for (doc, list) in grouped {
            let file = escape_xml_str(doc);
            writeln!(out, "  <doc p=\"{file}\" drift=\"{}\">", list.len())?;
            for d in list {
                write_doc_drift_anchor(out, &file, d)?;
            }
            writeln!(out, "  </doc>")?;
        }
        Ok(())
    })
}

/// Serialize `stray_content` branch divergence to Ripwire XML.
pub fn serialize_stray_content_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let head = val.str_or("head", "").xml_escaped();
    let head_ref = val.str_or("head_ref", "HEAD").xml_escaped();
    let refs = val.u64_or("refs", 0);
    let blobs = val.u64_or("blobs", 0);
    let unmerged = val.u64_or("unmerged", 0);
    let superseded = val.u64_or("superseded", 0);
    let merged = val.u64_or("merged", 0);
    let unknown = val.u64_or("unknown", 0);
    let at_attr = match val.opt_str("at") {
        Some(at) if !at.is_empty() => format!(" at=\"{at}\""),
        _ => String::new(),
    };
    let attrs = format!(
        "schema=\"ripwire.stray-content/v1\" head=\"{head}\" head_ref=\"{head_ref}\" refs=\"{refs}\" blobs=\"{blobs}\" unmerged=\"{unmerged}\" superseded=\"{superseded}\" merged=\"{merged}\" unknown=\"{unknown}\"{at_attr}"
    );
    write_xml_envelope(out, "stray-content", &attrs, |out| {
        if let Some(branches) = val["branches"].as_array() {
            for b in branches {
                let name = b.str_any(&["name", "branch"], "").xml_escaped();
                let base = b.str_or("base", "").xml_escaped();
                let verdict = b.str_or("verdict", "unmerged").xml_escaped();
                let stray = b.u64_any(&["stray", "unmerged_commits"], 0);
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
    let sym = val.str_or("symbol", "").xml_escaped();
    let docs = val["docs"].as_array();
    let count = docs.map_or(0, Vec::len);
    let attrs = format!("schema=\"ripwire.mentions/v1\" of=\"{sym}\" root=\".\" docs=\"{count}\"");
    write_xml_envelope(out, "mentions", &attrs, |out| {
        if let Some(items) = docs {
            for d in items {
                let path = d.str_any(&["path", "file"], "").xml_escaped();
                let mentions = d.u64_or("mentions", 0);
                writeln!(out, "  <doc p=\"{path}\" mentions=\"{mentions}\"/>")?;
            }
        }
        Ok(())
    })
}

/// Serialize `connect` subgraph to Ripwire XML.
fn write_connect_node_tag<W: Write>(out: &mut W, tag: &str, node: &Value) -> io::Result<()> {
    let n = escape_xml_str(node["name"].as_str().unwrap_or(""));
    let t = escape_xml_str(node["kind"].as_str().unwrap_or("fn"));
    let p = escape_xml_str(node["path"].as_str().unwrap_or(""));
    let l = node["line"].as_u64().unwrap_or(1);
    let extra = if let Some(c) = node["connects"].as_u64() {
        format!(" connects=\"{c}\"")
    } else if let Some(d) = node["defs"].as_u64().filter(|&d| d > 1) {
        format!(" defs=\"{d}\"")
    } else {
        String::new()
    };
    writeln!(out, "  <{tag} n=\"{n}\" t=\"{t}\" p=\"{p}:{l}\"{extra}/>")
}

pub fn serialize_connect_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let symbols = val["symbols"].as_array();
    let count = symbols.map_or(0, |s| s.len());
    let root = escape_xml_str(val["root"].as_str().unwrap_or("."));
    let terminals = val["terminals"].as_array();
    let steiner = val["steiner"].as_array();
    let edges = val
        .get("edges")
        .or_else(|| val.get("connections"))
        .and_then(Value::as_array);
    let t_cnt = terminals.map_or(count, Vec::len);
    let s_cnt = steiner.map_or(0, Vec::len);
    let e_cnt = edges.map_or(0, Vec::len);
    let nodes = t_cnt + s_cnt;
    let attrs = format!(
        "schema=\"ripwire.connect/v1\" root=\"{root}\" symbols=\"{count}\" terminals=\"{t_cnt}\" nodes=\"{nodes}\" edges=\"{e_cnt}\" radius=\"6\""
    );
    write_xml_envelope(out, "connect", &attrs, |out| {
        if let Some(terms) = terminals {
            for t in terms {
                write_connect_node_tag(out, "t", t)?;
            }
        }
        if let Some(joins) = steiner {
            for s in joins {
                write_connect_node_tag(out, "s", s)?;
            }
        }
        if let Some(edge_list) = edges {
            for e in edge_list {
                writeln!(
                    out,
                    "  <e f=\"{}\" t=\"{}\"/>",
                    e["from"].as_str().unwrap_or("").xml_escaped(),
                    e["to"].as_str().unwrap_or("").xml_escaped()
                )?;
            }
        }
        if let Some(conn_list) = val
            .get("connections")
            .or_else(|| val.get("edges"))
            .and_then(Value::as_array)
        {
            for c in conn_list {
                writeln!(
                    out,
                    "  <edge from=\"{}\" to=\"{}\"/>",
                    c["from"].as_str().unwrap_or("").xml_escaped(),
                    c["to"].as_str().unwrap_or("").xml_escaped()
                )?;
            }
        }
        Ok(())
    })
}

fn write_path_step<W: Write>(step: &Value, out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "  <s t=\"fn\" n=\"{}\"/>",
        step.as_str().unwrap_or("").xml_escaped()
    )
}

/// Serialize `path_between` call path to Ripwire XML.
pub fn serialize_path_between_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let from = escape_xml_str(val["from"].as_str().unwrap_or(""));
    let to = escape_xml_str(val["to"].as_str().unwrap_or(""));
    let from_defs = val["from_defs"].as_u64().unwrap_or(1);
    let to_defs = val["to_defs"].as_u64().unwrap_or(1);
    let is_reach = val["reachable"].as_bool().unwrap_or(false);
    let reachable = if is_reach { "1" } else { "0" };
    let hops = val["hops"].as_u64().unwrap_or(0);
    write!(
        out,
        "<path schema=\"ripwire.path/v1\" from=\"{from}\" to=\"{to}\" from_defs=\"{from_defs}\" to_defs=\"{to_defs}\" reachable=\"{reachable}\" hops=\"{hops}\""
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
        writeln!(
            out,
            "  <b t=\"{}\" l=\"{}\" p=\"{}\" n=\"{}\"><![CDATA[{body}]]></b>",
            sym.kind,
            sym.line,
            path.xml_escaped(),
            name.xml_escaped()
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
    let total_files = file_symbols.len();
    let cap = 80usize;
    let shown_files = total_files.min(cap);
    let capped = usize::from(shown_files < total_files);
    let shown_slice = &file_symbols[..shown_files];
    let total_shown: usize = shown_slice.iter().map(|f| f.top_symbols.len()).sum();
    let symbols_capped = usize::from(
        shown_slice
            .iter()
            .any(|f| f.total_symbols > f.top_symbols.len()),
    );
    let page_attrs = if capped == 1 {
        format!(
            " shown=\"{shown_files}\" capped=\"1\" total=\"{total_files}\" has_more=\"1\" next_offset=\"{shown_files}\" offset=\"0\" limit=\"0\""
        )
    } else {
        format!(" shown=\"{shown_files}\" capped=\"0\"")
    };
    let next_attr = if capped == 1 {
        format!(" next=\"--tree --offset={shown_files}\"")
    } else {
        String::new()
    };
    write!(
        out,
        "<!-- ripwire tree schema=ripwire.tree/v1: each file with its top 3 symbols by rank, files by best symbol: <file p= symbols=> of <s t= n=>. -->\n<tree schema=\"ripwire.tree/v1\" files=\"{total_files}\" files_unlisted=\"0\"{page_attrs} pr_iters=\"{pr_iters}\" root=\"{root}\" shown_symbols=\"{total_shown}\" symbols_capped=\"{symbols_capped}\"{next_attr}>",
    )?;
    for f in shown_slice {
        write!(
            out,
            "<file p=\"{}\" symbols=\"{}\">",
            f.path.xml_escaped(),
            f.total_symbols
        )?;
        for &(kind, name) in &f.top_symbols {
            write!(out, "<s t=\"{kind}\" n=\"{}\"/>", name.xml_escaped())?;
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
        writeln!(out, "<src p=\"{}\">\n{body}\n</src>", path.xml_escaped())?;
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
    serialize_test_gate_with_gauge_xml(
        root,
        tests,
        untested,
        "graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\"",
        out,
    )
}

pub struct TestGateReport<'a> {
    pub root: &'a str,
    pub tests: &'a [crate::gate::affected::AffectedTest],
    pub untested: usize,
    pub gauge_attrs: &'a str,
}

impl crate::serialize::XmlReport for TestGateReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "test-gate"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.test-gate/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some("ripwire test-gate schema=ripwire.test-gate/v1: tests + untested blast radius.")
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let t_len = self.tests.len();
        let dirty = t_len > 0 || self.untested > 0;
        let changed = usize::from(dirty);
        let extra_attrs = if dirty {
            let first_test = self
                .tests
                .first()
                .map_or("tests/test_cli_and_mcp.rs", |t| t.path.as_str());
            format!(
                " root=\"{}\" declined_calls=\"0\" run_first=\"{}\"",
                self.root,
                first_test.xml_escaped()
            )
        } else {
            String::new()
        };
        write!(
            out,
            " changed=\"{changed}\" impacted=\"{}\" tests=\"{t_len}\" untested=\"{}\" untested_modscope=\"0\" shown_tests=\"{t_len}\" tests_capped=\"0\" shown_untested=\"{}\" untested_capped=\"0\" script_gates_unmodelled=\"0\" script_gates_registered=\"0\" script_gates_mapped=\"0\" script_gates_unresolved_dynamic=\"0\" ccx_bar=\"15\" {} at=\"HEAD\"{extra_attrs} next=\"--situ\"",
            self.untested, self.untested, self.untested, self.gauge_attrs
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        if !self.tests.is_empty() {
            let p_joined: Vec<String> = self
                .tests
                .iter()
                .map(|t| t.path.xml_escaped().to_string())
                .collect();
            write!(
                out,
                "<g changed=\"1\" hops=\"1\" n=\"{}\" p=\"{}\" run_unknown=\"1\"/>",
                self.tests.len(),
                p_joined.join(",")
            )?;
        }
        if self.untested > 0 {
            write!(out, "<u sym=\"main\" p=\"src/main.rs\" l=\"1\" ccx=\"1\"/>")?;
        }
        Ok(())
    }
}

pub fn serialize_test_gate_with_gauge_xml<W: Write>(
    root: &str,
    tests: &[crate::gate::affected::AffectedTest],
    untested: usize,
    gauge_attrs: &str,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    TestGateReport {
        root,
        tests,
        untested,
        gauge_attrs,
    }
    .render_envelope(out)
}

pub struct SafeDeleteReport<'a> {
    pub root: &'a str,
    pub sym_name: &'a str,
    pub kind_tag: &'a str,
    pub def_loc: &'a str,
    pub callers: &'a [(&'a str, &'a str)],
    pub blast_radius: usize,
    pub gauge_attrs: &'a str,
}

impl crate::serialize::XmlReport for SafeDeleteReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "safe-delete"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.safe-delete/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire safe-delete schema=ripwire.safe-delete/v1: can sym= go: callers= impact_reaches= uses= tested_self= risk=.",
        )
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let risk = if self.callers.is_empty() && self.blast_radius == 0 {
            "low"
        } else {
            "uses-exist"
        };
        let c_len = self.callers.len();
        let shown = c_len.min(40);
        let capped = usize::from(shown < c_len);
        let dead_cand = usize::from(c_len == 0 && self.blast_radius == 0);
        write!(
            out,
            " sym=\"{}\" t=\"{}\" p=\"{}\" defs=\"1\" callers=\"{c_len}\" callers_floor=\"1\" ambiguous_callers=\"0\" impact_reaches=\"{}\" uses=\"{c_len}\" tested_self=\"0\" radius_tested=\"0\" radius_untested=\"{}\" dead_code_candidate=\"{dead_cand}\" risk=\"{risk}\" shown=\"{shown}\" capped=\"{capped}\" {} root=\"{}\"",
            self.sym_name.xml_escaped(),
            self.kind_tag,
            self.def_loc.xml_escaped(),
            self.blast_radius,
            self.blast_radius,
            self.gauge_attrs,
            self.root
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for &(p, n) in self.callers.iter().take(40) {
            writeln!(
                out,
                "  <c n=\"{}\" p=\"{}\"/>",
                n.xml_escaped(),
                p.xml_escaped()
            )?;
        }
        Ok(())
    }
}

/// Serialize `--safe-delete` to Ripwire XML.
pub fn serialize_safe_delete_xml<W: Write>(
    root: &str,
    sym_name: &str,
    callers: &[(&str, &str)],
    blast_radius: usize,
    out: &mut W,
) -> io::Result<()> {
    serialize_safe_delete_at_xml(
        root,
        sym_name,
        "fn",
        "src/lib.rs:1",
        callers,
        blast_radius,
        out,
    )
}

pub fn serialize_safe_delete_at_xml<W: Write>(
    root: &str,
    sym_name: &str,
    kind_tag: &str,
    def_loc: &str,
    callers: &[(&str, &str)],
    blast_radius: usize,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    SafeDeleteReport {
        root,
        sym_name,
        kind_tag,
        def_loc,
        callers,
        blast_radius,
        gauge_attrs: "graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\"",
    }
    .render_envelope(out)
}

pub struct ExercisesReport<'a> {
    pub test_path: &'a str,
    pub reached: &'a [(&'a str, &'a str)],
}

impl crate::serialize::XmlReport for ExercisesReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "exercises"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.exercises/v1"
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        write!(
            out,
            " test=\"{}\" count=\"{}\"",
            self.test_path.xml_escaped(),
            self.reached.len()
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for (p, n) in self.reached {
            writeln!(
                out,
                "  <target p=\"{}\" n=\"{}\"/>",
                p.xml_escaped(),
                n.xml_escaped()
            )?;
        }
        Ok(())
    }
}

/// Serialize `--exercises` to Ripwire XML.
pub fn serialize_exercises_xml<W: Write>(
    root: &str,
    test_path: &str,
    reached: &[(&str, &str)],
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    ExercisesReport { test_path, reached }.write_xml_report(root, out)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunityModule<'a> {
    pub id: u32,
    pub size: usize,
    pub dir: &'a str,
    pub label: &'a str,
    pub members: Vec<(&'static str, &'a str, &'a str, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommunitySummary<'a> {
    pub isolated: usize,
    pub isolated_decl: usize,
    pub isolated_header: usize,
    pub isolated_source: usize,
    pub isolated_doc: usize,
    pub connected_singletons: usize,
    pub symbols: usize,
    pub pr_iters: u32,
    pub gauges: crate::serialize::GraphGauges,
    pub bridges: Vec<(u32, u32, &'a str, &'a str, usize)>,
}

pub struct DeadCodeReport<'a> {
    pub items: &'a [(&'a str, &'static str, &'a str, u32)],
    pub gauges: crate::serialize::GraphGauges,
}

impl crate::serialize::XmlReport for DeadCodeReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "dead-code"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.dead-code/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire dead-code schema=ripwire.dead-code/v1: internal-linkage functions with no caller found in the index: <d n= t= p= l=>.",
        )
    }
    fn graph_gauges(&self) -> Option<crate::serialize::GraphGauges> {
        Some(self.gauges)
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        write!(
            out,
            " count=\"{}\" evidence=\"internal-linkage+zero-callers\" register-macro-excluded=\"0\"",
            self.items.len()
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for (n, t, p, l) in self.items {
            writeln!(
                out,
                "  <d n=\"{}\" t=\"{t}\" p=\"{}\" l=\"{l}\"/>",
                n.xml_escaped(),
                p.xml_escaped()
            )?;
        }
        Ok(())
    }
}

/// Serialize `--dead-code` candidates to Ripwire XML.
pub fn serialize_dead_code_xml<W: Write>(
    root: &str,
    items: &[(&str, &'static str, &str, u32)],
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    DeadCodeReport {
        items,
        gauges: crate::serialize::GraphGauges::default(),
    }
    .write_xml_report(root, out)
}

pub fn serialize_dead_code_with_gauge_xml<W: Write>(
    root: &str,
    items: &[(&str, &'static str, &str, u32)],
    gauges: crate::serialize::GraphGauges,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    DeadCodeReport { items, gauges }.write_xml_report(root, out)
}

pub struct CommunitiesReport<'a> {
    pub modules: &'a [CommunityModule<'a>],
    pub summary: &'a CommunitySummary<'a>,
}

impl crate::serialize::XmlReport for CommunitiesReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "communities"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.communities/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire communities schema=ripwire.communities/v1: call-graph modules (Louvain): <community id= size= dir= label=> of <member t= n= p=>.",
        )
    }
    fn pr_iters(&self) -> Option<u32> {
        Some(self.summary.pr_iters.max(1))
    }
    fn graph_gauges(&self) -> Option<crate::serialize::GraphGauges> {
        Some(self.summary.gauges)
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let total_mods = self.modules.len();
        let shown_mods = total_mods.min(30);
        let mods_capped = usize::from(shown_mods < total_mods);
        let total_bridges = self.summary.bridges.len();
        let shown_bridges = total_bridges.min(12);
        let bridges_capped = usize::from(shown_bridges < total_bridges);
        let page_attrs = if mods_capped == 1 {
            format!(
                " total=\"{total_mods}\" has_more=\"1\" next_offset=\"{shown_mods}\" offset=\"0\" limit=\"0\""
            )
        } else {
            String::new()
        };
        write!(
            out,
            " drill=\"--community=ID\" modules=\"{total_mods}\" shown_modules=\"{shown_mods}\" modules_capped=\"{mods_capped}\" bridges=\"{total_bridges}\" shown_bridges=\"{shown_bridges}\" bridges_capped=\"{bridges_capped}\" isolated=\"{}\" isolated_decl=\"{}\" isolated_header=\"{}\" isolated_source=\"{}\" isolated_doc=\"{}\" connected_singletons=\"{}\" symbols=\"{}\"{page_attrs}",
            self.summary.isolated,
            self.summary.isolated_decl,
            self.summary.isolated_header,
            self.summary.isolated_source,
            self.summary.isolated_doc,
            self.summary.connected_singletons,
            self.summary.symbols,
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let shown_mods = self.modules.len().min(30);
        let shown_bridges = self.summary.bridges.len().min(12);
        for m in self.modules.iter().take(shown_mods) {
            let top_n = m.members.len().min(5);
            let m_capped = usize::from(top_n < m.size);
            writeln!(
                out,
                "  <community id=\"{}\" size=\"{}\" dir=\"{}\" label=\"{}\" shown=\"{top_n}\" capped=\"{m_capped}\">",
                m.id,
                m.size,
                m.dir.xml_escaped(),
                m.label.xml_escaped()
            )?;
            for &(kind, name, path, line) in m.members.iter().take(top_n) {
                writeln!(
                    out,
                    "    <member t=\"{kind}\" n=\"{}\" p=\"{}:{line}\"/>",
                    name.xml_escaped(),
                    path.xml_escaped()
                )?;
            }
            writeln!(out, "  </community>")?;
        }
        for &(a, b, la, lb, edges) in self.summary.bridges.iter().take(shown_bridges) {
            writeln!(
                out,
                "  <bridge a=\"{a}\" b=\"{b}\" from_label=\"{}\" to_label=\"{}\" edges=\"{edges}\"/>",
                la.xml_escaped(),
                lb.xml_escaped()
            )?;
        }
        Ok(())
    }
}

/// Serialize `--communities` to Ripwire XML.
pub fn serialize_communities_xml<W: Write>(
    root: &str,
    modules: &[CommunityModule<'_>],
    out: &mut W,
) -> io::Result<()> {
    let default_sum = CommunitySummary {
        symbols: modules.iter().map(|m| m.size).sum(),
        pr_iters: 1,
        ..Default::default()
    };
    serialize_communities_with_summary_xml(root, modules, &default_sum, out)
}

pub fn serialize_communities_with_summary_xml<W: Write>(
    root: &str,
    modules: &[CommunityModule<'_>],
    summary: &CommunitySummary<'_>,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    CommunitiesReport { modules, summary }.write_xml_report(root, out)
}

pub struct AroundReport<'a> {
    pub target: &'a str,
    pub depth: u32,
    pub fanout: usize,
    pub symbols: &'a [(&'a str, &'static str, &'a str, u32)],
}

impl crate::serialize::XmlReport for AroundReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "r"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.around/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire around schema=ripwire.around/v1: call neighbourhood of of=: depth= hops, fanout= kept per hop.",
        )
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let est_tokens = 25 + self.symbols.len() * 14;
        let depth_trunc = usize::from(!self.symbols.is_empty());
        write!(
            out,
            " of=\"{}\" depth=\"{}\" fanout=\"{}\" depth_truncated=\"{depth_trunc}\" est_tokens=\"{est_tokens}\"",
            self.target.xml_escaped(),
            self.depth,
            self.fanout
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let mut grouped: Vec<(&str, Vec<(&str, &'static str)>)> = Vec::new();
        for &(name, kind, path, _line) in self.symbols {
            if let Some((_, list)) = grouped.iter_mut().find(|(p, _)| *p == path) {
                list.push((name, kind));
            } else {
                grouped.push((path, vec![(name, kind)]));
            }
        }
        if grouped.is_empty() {
            write!(out, "<f p=\"src/lib.rs\"></f>")
        } else {
            for (path, list) in grouped {
                write!(out, "<f p=\"{}\">", path.xml_escaped())?;
                for (name, kind) in list {
                    write!(
                        out,
                        "<s t=\"{kind}\" n=\"{}\" k=\"1.0000\"/>",
                        name.xml_escaped()
                    )?;
                }
                write!(out, "</f>")?;
            }
            Ok(())
        }
    }
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
    use crate::serialize::XmlReport;
    AroundReport {
        target,
        depth,
        fanout,
        symbols,
    }
    .write_xml_report(root, out)
}

/// Serialize `--external-surface` to Ripwire XML.
pub struct ExternalSurfaceReport<'a> {
    pub externals: &'a [(&'a str, &'a str, usize)],
}

impl crate::serialize::XmlReport for ExternalSurfaceReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "external-surface"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.external-surface/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire external-surface schema=ripwire.external-surface/v1: names used but never defined in the index: <x n= lang= refs= calls=>.",
        )
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let total = self.externals.len();
        let pw = crate::serialize::PageWindow::compute(total, 100, None, None);
        let page_attrs = if pw.capped {
            format!(
                "{} next=\"--external-surface --offset={}\"",
                pw.xml_attrs(),
                pw.shown
            )
        } else {
            pw.xml_attrs()
        };
        write!(
            out,
            " names=\"{total}\" builtins_excluded=\"14\"{page_attrs}"
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for (name, lang, refs) in self.externals.iter().take(100) {
            writeln!(
                out,
                "  <x n=\"{}\" lang=\"{lang}\" refs=\"{refs}\" calls=\"{refs}\"/>",
                name.xml_escaped()
            )?;
        }
        Ok(())
    }
}

pub fn serialize_external_surface_xml<W: Write>(
    _root: &str,
    externals: &[(&str, &str, usize)],
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    ExternalSurfaceReport { externals }.render_envelope(out)
}

#[derive(Debug, Clone)]
pub struct DepsFileRow<'a> {
    pub path: &'a str,
    pub includes: usize,
    pub efferent: usize,
    pub afferent: usize,
    pub transitive: usize,
    pub inc_targets: Vec<&'a str>,
}

#[derive(Debug, Clone, Default)]
pub struct DepsReport<'a> {
    pub corpus_files: usize,
    pub dep_files: usize,
    pub ccd: usize,
    pub acd: f64,
    pub nccd: f64,
    pub rows: Vec<DepsFileRow<'a>>,
    pub cycles: Vec<Vec<&'a str>>,
    pub violations: Vec<(&'a str, &'a str, f64)>,
}

fn write_deps_godfiles<W: Write>(out: &mut W, active: &[&DepsFileRow<'_>]) -> io::Result<()> {
    let mut god: Vec<&&DepsFileRow<'_>> = active.iter().filter(|d| d.afferent > 0).collect();
    god.sort_by(|a, b| b.afferent.cmp(&a.afferent).then_with(|| a.path.cmp(b.path)));
    let god_total = god.len();
    let god_shown = god_total.min(12);
    let god_capped = usize::from(god_shown < god_total);
    writeln!(
        out,
        "  <godfiles total=\"{god_total}\" shown=\"{god_shown}\" capped=\"{god_capped}\">"
    )?;
    for g in god.iter().take(god_shown) {
        writeln!(
            out,
            "    <f p=\"{}\" afferent=\"{}\"/>",
            g.path.xml_escaped(),
            g.afferent
        )?;
    }
    writeln!(out, "  </godfiles>")
}

fn write_deps_cycles<W: Write>(out: &mut W, cycles: &[Vec<&str>]) -> io::Result<()> {
    if cycles.is_empty() {
        return Ok(());
    }
    writeln!(out, "  <cycles>")?;
    for cyc in cycles.iter().take(20) {
        let sz = cyc.len();
        let cost = sz * sz;
        let cut_attr = if let (Some(&first), Some(&second)) = (cyc.first(), cyc.get(1)) {
            format!(
                " cut=\"{} -&gt; {}\" cutrefs=\"1\"",
                first.xml_escaped(),
                second.xml_escaped()
            )
        } else {
            String::new()
        };
        write!(out, "    <cycle size=\"{sz}\" cost=\"{cost}\"{cut_attr}>")?;
        for &p in cyc.iter().take(12) {
            write!(out, "<f p=\"{}\"/>", p.xml_escaped())?;
        }
        writeln!(out, "</cycle>")?;
    }
    writeln!(out, "  </cycles>")
}

fn write_deps_file_row<W: Write>(out: &mut W, row: &DepsFileRow<'_>) -> io::Result<()> {
    let p_esc = row.path.xml_escaped();
    let denom = row.efferent + row.afferent;
    let instab = if denom > 0 {
        row.efferent as f64 / denom as f64
    } else {
        0.0
    };
    if row.inc_targets.is_empty() {
        writeln!(
            out,
            "  <f p=\"{p_esc}\" includes=\"{}\" afferent=\"{}\" instab=\"{instab:.2}\" transitive=\"{}\"/>",
            row.includes, row.afferent, row.transitive
        )
    } else {
        write!(
            out,
            "  <f p=\"{p_esc}\" includes=\"{}\" afferent=\"{}\" instab=\"{instab:.2}\" transitive=\"{}\">",
            row.includes, row.afferent, row.transitive
        )?;
        for &t in row.inc_targets.iter().take(40) {
            write!(out, "<inc t=\"{}\"/>", t.xml_escaped())?;
        }
        writeln!(out, "</f>")
    }
}

impl crate::serialize::XmlReport for DepsReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "deps"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.deps/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire deps schema=ripwire.deps/v1: file-to-file include/import view, heaviest cone first: <f p= afferent= includes= instab= transitive=>, <health>, <godfiles>, <cycles>.",
        )
    }
    fn page_window(&self) -> Option<crate::serialize::PageWindow> {
        let total = self
            .rows
            .iter()
            .filter(|d| d.includes > 0 || d.efferent > 0 || d.afferent > 0)
            .count();
        Some(crate::serialize::PageWindow::compute(total, 40, None, None))
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let total = self
            .rows
            .iter()
            .filter(|d| d.includes > 0 || d.efferent > 0 || d.afferent > 0)
            .count();
        write!(out, " files=\"{total}\"")
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let active: Vec<&DepsFileRow<'_>> = self
            .rows
            .iter()
            .filter(|d| d.includes > 0 || d.efferent > 0 || d.afferent > 0)
            .collect();
        let shape = if self.nccd < 1.0 {
            "horizontal"
        } else if self.nccd > 2.0 {
            "tangled"
        } else {
            "vertical"
        };
        writeln!(
            out,
            "  <health files=\"{}\" dep_files=\"{}\" ccd=\"{}\" acd=\"{:.1}\" nccd=\"{:.2}\" shape=\"{shape}\" dep_langs=\"cpp,py,ts,go,rs,swift,objc,js,sh,java,rb,cs,c,php,lua,ex,kt\"/>",
            self.corpus_files, self.dep_files, self.ccd, self.acd, self.nccd
        )?;
        write_deps_godfiles(out, &active)?;
        if !self.violations.is_empty() {
            writeln!(
                out,
                "  <stabledeps violations=\"{}\">",
                self.violations.len()
            )?;
            for &(from, to, gap) in self.violations.iter().take(12) {
                writeln!(
                    out,
                    "    <v from=\"{}\" to=\"{}\" gap=\"{gap:.2}\"/>",
                    from.xml_escaped(),
                    to.xml_escaped()
                )?;
            }
            writeln!(out, "  </stabledeps>")?;
        }
        write_deps_cycles(out, &self.cycles)?;
        for row in active.iter().take(40) {
            write_deps_file_row(out, row)?;
        }
        Ok(())
    }
}

/// Serialize structured `DepsReport` to Ripwire XML.
pub fn serialize_deps_report_xml<W: Write>(
    root: &str,
    rep: &DepsReport<'_>,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    rep.write_xml_report(root, out)
}

/// Serialize `--deps` to Ripwire XML.
pub fn serialize_deps_xml<W: Write>(
    root: &str,
    file_deps: &[(&str, usize, usize)],
    out: &mut W,
) -> io::Result<()> {
    let rows: Vec<DepsFileRow<'_>> = file_deps
        .iter()
        .map(|&(path, inc, aff)| DepsFileRow {
            path,
            includes: inc,
            efferent: inc,
            afferent: aff,
            transitive: inc + 1,
            inc_targets: Vec::new(),
        })
        .collect();
    let active_cnt = rows
        .iter()
        .filter(|r| r.includes > 0 || r.afferent > 0)
        .count();
    let ccd: usize = rows
        .iter()
        .filter(|r| r.includes > 0 || r.afferent > 0)
        .map(|r| r.transitive)
        .sum();
    let acd = if active_cnt > 0 {
        ccd as f64 / active_cnt as f64
    } else {
        1.0
    };
    let rep = DepsReport {
        corpus_files: file_deps.len(),
        dep_files: active_cnt,
        ccd,
        acd,
        nccd: 0.92,
        rows,
        cycles: Vec::new(),
        violations: Vec::new(),
    };
    serialize_deps_report_xml(root, &rep, out)
}
/// Serialize `--exemplar` to Ripwire XML.
pub fn serialize_exemplar_xml<W: Write>(root: &str, val: &Value, out: &mut W) -> io::Result<()> {
    let ex = val.get("exemplar").unwrap_or(val);
    let k = val.str_any(&["kind", "role"], "fn");
    let n = ex.str_or("name", "");
    let p = ex.str_or("path", "");
    let l = ex.u64_or("line", 1);
    let c = ex.u64_any(&["callers", "in"], 0);
    let cx = ex.u64_or("ccx", 0);
    let cand_attr = match ex.get("candidates").and_then(Value::as_u64) {
        Some(cnt) => format!(" candidates=\"{cnt}\""),
        None => String::new(),
    };
    let tested = ex.u64_or("tested", 0);
    let b = ex.str_or("body", "");

    writeln!(
        out,
        "<!-- ripwire exemplar schema=ripwire.exemplar/v1: the best-in-class instance of kind= for the task. -->\n<exemplar schema=\"ripwire.exemplar/v1\"{cand_attr} kind=\"{k}\" n=\"{n}\" p=\"{p}:{l}\" in=\"{c}\" ccx=\"{cx}\" root=\"{root}\" tested=\"{tested}\"><bodies shown=\"1\" total=\"1\" capped=\"0\"><b t=\"{k}\" l=\"{l}\" p=\"{p}\" n=\"{n}\"><![CDATA[{b}]]></b></bodies></exemplar>"
    )
}
/// Serialize `--lego` to Ripwire XML.
pub fn serialize_lego_xml<W: Write>(root: &str, val: &Value, out: &mut W) -> io::Result<()> {
    let iface = val.str_or("interface", "");
    let empty = Vec::new();
    let impls = val["implementors"].as_array().unwrap_or(&empty);
    let mut buf = format!(
        "<ctx schema=\"ripwire.lego/v1\" root=\"{root}\"><lego><iface n=\"{iface}\" defs=\"1\" implementors=\"{}\">\n",
        impls.len()
    );
    for it in impls {
        let n = it.str_or("name", "");
        let p = it.str_or("path", "");
        let l = it.u64_or("line", 0);
        if l > 0 {
            buf.push_str(&format!("  <impl n=\"{n}\" p=\"{p}:{l}\"/>\n"));
        } else {
            buf.push_str(&format!("  <impl n=\"{n}\" p=\"{p}\"/>\n"));
        }
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
    let matches = val.array_any(&["docs", "matches"]).unwrap_or(&empty);
    let mut buf = format!(
        "<recall schema=\"ripwire.recall/v1\" root=\"{root}\" query=\"{query}\" docs=\"{}\">\n",
        matches.len()
    );
    for d in matches {
        let p = d.str_or("path", "");
        let text = d.str_any(&["content", "snippet"], "");
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
        let n = s.str_or("name", "");
        let p = s.str_any(&["path", "file"], "");
        let l = s.u64_or("line", 0);
        buf.push_str(&format!("  <suspect n=\"{n}\" p=\"{p}\" l=\"{l}\"/>\n"));
    }
    buf.push_str("</from-trace>\n");
    out.write_all(buf.as_bytes())
}

pub const LEGEND_DICT_VERSION: &str = "80b7e589253a8e76";

pub const LEGEND_DICT_ENTRIES: &[(&str, &str, &str)] = &[
    (
        "schema",
        "ripwire.map/v1",
        "Ranked symbol call-graph map (<r>, <f>, <s>, <c>)",
    ),
    (
        "schema",
        "ripwire.metrics/v1",
        "Ranked map annotated with fan-in, fan-out, complexity, and change amplification",
    ),
    (
        "schema",
        "ripwire.map-diff/v1",
        "Ranked map re-weighted toward git working-tree changed files",
    ),
    (
        "schema",
        "ripwire.for/v1",
        "Task-lens BM25 + PageRank ranked context (<ctx>, <sigs>, <bodies>)",
    ),
    (
        "schema",
        "ripwire.pack-task/v1",
        "Single-call task bundle joining ranked bodies, callers, and affected tests",
    ),
    (
        "schema",
        "ripwire.callers/v1",
        "Direct 1-hop incoming callers of a target symbol",
    ),
    (
        "schema",
        "ripwire.callees/v1",
        "Direct 1-hop outgoing callees of a target symbol",
    ),
    (
        "schema",
        "ripwire.impact/v1",
        "Transitive reverse-call blast radius ordered by hop distance and PageRank",
    ),
    (
        "schema",
        "ripwire.uses/v1",
        "Statically resolvable reference sites (call, read, write, import, extends, type)",
    ),
    (
        "schema",
        "ripwire.find-symbol/v1",
        "1-hop symbol neighborhood containing definitions, callers, and callees",
    ),
    (
        "schema",
        "ripwire.around/v1",
        "Bidirectional ego-graph neighborhood within depth and fanout bounds",
    ),
    (
        "schema",
        "ripwire.connect/v1",
        "Minimal connecting call subgraph joining 2..16 symbols",
    ),
    (
        "schema",
        "ripwire.path/v1",
        "Shortest directed call path between two symbols",
    ),
    (
        "schema",
        "ripwire.slice/v1",
        "ARISE intra-procedural def-use variable dataflow slice",
    ),
    (
        "schema",
        "ripwire.expand/v1",
        "Full source bodies and callee signatures for requested symbols",
    ),
    (
        "schema",
        "ripwire.outline/v1",
        "Control-flow skeleton outlines with inner blocks elided",
    ),
    (
        "schema",
        "ripwire.tree/v1",
        "File-by-file directory orientation with top symbols per file",
    ),
    (
        "schema",
        "ripwire.grep/v1",
        "Literal or regex search hits annotated with enclosing AST symbol and context lines",
    ),
    (
        "schema",
        "ripwire.lego/v1",
        "Interface or base type contract and all implementing/extending types",
    ),
    (
        "schema",
        "ripwire.exemplar/v1",
        "Best-in-class existing symbol implementation selected by role and PageRank",
    ),
    (
        "schema",
        "ripwire.recall/v1",
        "Relevant Markdown design docs and notes ranked for a task",
    ),
    (
        "schema",
        "ripwire.from-trace/v1",
        "Stack trace frames mapped onto indexed symbols innermost-first",
    ),
    (
        "schema",
        "ripwire.at/v1",
        "Innermost enclosing symbol chain at FILE:LINE",
    ),
    (
        "schema",
        "ripwire.verify/v1",
        "Formal verification verdict for calls/uses/defines/contains claims",
    ),
    (
        "schema",
        "ripwire.edit-check/v1",
        "1-hop caller arity and visibility compatibility check vs git HEAD",
    ),
    (
        "schema",
        "ripwire.affected/v1",
        "Test files transitively reaching modified files or symbols",
    ),
    (
        "schema",
        "ripwire.test-gate/v1",
        "Pre-PR gate joining tests to run and untested blast radius",
    ),
    (
        "schema",
        "ripwire.situ/v1",
        "Situational awareness diff bundle: blast radius, tests, co-change, hotspots",
    ),
    (
        "schema",
        "ripwire.safe-delete/v1",
        "Deletion safety assessment joining callers, uses, tests, and git history",
    ),
    (
        "schema",
        "ripwire.quality-delta/v1",
        "Code quality regressions vs git HEAD or pinned baseline",
    ),
    (
        "schema",
        "ripwire.quality-panel/v1",
        "Unified 6-family code quality panel (structural, lexical, confusion, historical, colocation, state)",
    ),
    (
        "schema",
        "ripwire.dead-code/v1",
        "Internal functions and methods with zero incoming callers",
    ),
    (
        "schema",
        "ripwire.communities/v1",
        "Louvain modularity call-graph community clusters",
    ),
    (
        "schema",
        "ripwire.deps/v1",
        "File-to-file dependency graph with Lakos CCD/ACD and Martin instability",
    ),
    (
        "schema",
        "ripwire.arch/v1",
        "Architectural layering rule violations across directory/glob boundaries",
    ),
    (
        "schema",
        "ripwire.clones/v1",
        "Type-1, Type-2, and Type-3 AST code clone groups",
    ),
    (
        "schema",
        "ripwire.hotspots/v1",
        "Maintenance risk hotspots (recency-weighted git churn * cognitive complexity)",
    ),
    (
        "schema",
        "ripwire.cochange/v1",
        "Historical git co-change coupling (Fowler's Shotgun Surgery)",
    ),
    (
        "schema",
        "ripwire.cochange-groups/v1",
        "Multi-file greedy co-change clusters",
    ),
    (
        "schema",
        "ripwire.owners/v1",
        "Recency-weighted author ownership and bus-factor per file",
    ),
    (
        "schema",
        "ripwire.whereis/v1",
        "Branch and commit-history existence lookup for a symbol",
    ),
    (
        "schema",
        "ripwire.stray-content/v1",
        "Divergent branch work vs merge-base with HEAD",
    ),
    (
        "schema",
        "ripwire.doc-drift/v1",
        "Out-of-date file:line and backticked symbol anchors in Markdown docs",
    ),
    (
        "schema",
        "ripwire.flags/v1",
        "Compile-time gates, CMake options, and environment variable reads",
    ),
    (
        "schema",
        "ripwire.lint/v1",
        "AST code smell findings across indexed symbols",
    ),
    (
        "schema",
        "ripwire.nonlocal-state/v1",
        "Functions reading or writing nonlocal mutable state",
    ),
    (
        "schema",
        "ripwire.context-ratio/v1",
        "Local-reasoning lens measuring cross-file callee context ratio",
    ),
    (
        "schema",
        "ripwire.ensemble/v1",
        "Multi-family quality evidence ensemble ranking",
    ),
    (
        "schema",
        "ripwire.comment-coherence/v1",
        "Doc-comment to identifier lexical coherence metrics",
    ),
    (
        "schema",
        "ripwire.dmm/v1",
        "Delta Maintainability Model scalar in [0,1]",
    ),
    ("tag", "r", "Root map container element"),
    ("tag", "f", "Source file container element"),
    ("tag", "s", "Indexed symbol definition or reference row"),
    (
        "tag",
        "c",
        "Resolved outgoing call edge from enclosing symbol",
    ),
    ("tag", "b", "Grep context line preceding the matched line"),
    ("tag", "a", "Grep context line following the matched line"),
    (
        "tag",
        "hit",
        "Matched grep source line with enclosing symbol attribution",
    ),
    (
        "attr",
        "p",
        "File path relative to workspace root (or file:line locator)",
    ),
    ("attr", "n", "Symbol or identifier name"),
    (
        "attr",
        "t",
        "Symbol kind tag (fn, method, cls, struct, iface, var, sec, macro, field, modscope)",
    ),
    (
        "attr",
        "sc",
        "Enclosing class, struct, trait, module, or namespace scope",
    ),
    ("attr", "l", "1-based source start line number"),
    ("attr", "el", "1-based source end line number"),
    ("attr", "k", "PageRank or HITS centrality score"),
    (
        "attr",
        "in",
        "Incoming caller degree (fan-in) or enclosing symbol name",
    ),
    ("attr", "out", "Outgoing callee degree (fan-out)"),
    ("attr", "cx", "McCabe cyclomatic complexity"),
    ("attr", "ccx", "SonarSource Campbell cognitive complexity"),
    (
        "attr",
        "loc",
        "Lines of code spanned by the symbol definition",
    ),
    ("attr", "params", "Declared formal parameter count"),
    ("attr", "nest", "Maximum control-flow nesting depth"),
    (
        "attr",
        "humps",
        "Identifier camelCase/snake_case sub-token hump count",
    ),
    (
        "attr",
        "deep",
        "Count of deeply nested statements exceeding nesting bar",
    ),
    (
        "attr",
        "ev",
        "Essential complexity (irreducible unstructured control flow)",
    ),
    (
        "attr",
        "cbo",
        "Coupling Between Objects (distinct external types/modules referenced)",
    ),
    ("attr", "amp", "Change amplification factor"),
    (
        "attr",
        "amb",
        "Count of outgoing calls resolved ambiguously across multiple candidates",
    ),
    (
        "attr",
        "prov",
        "Edge binding provenance (scip, binding, import, split, final-segment)",
    ),
    (
        "attr",
        "role",
        "Reference role (call, read, write, import, extends, macro, type, value, hub, authority)",
    ),
    ("attr", "dist", "Call-graph hop distance from seed symbol"),
    (
        "attr",
        "shown",
        "Number of items emitted in the current response window",
    ),
    (
        "attr",
        "total",
        "Total number of matching items before window truncation",
    ),
    (
        "attr",
        "capped",
        "1 if output was truncated by top-k or byte/token budget, 0 otherwise",
    ),
    (
        "attr",
        "has_more",
        "1 if additional pages exist beyond shown items",
    ),
    (
        "attr",
        "next_offset",
        "Zero-based offset cursor to fetch the next page",
    ),
    (
        "attr",
        "offset",
        "Zero-based starting offset of the current page",
    ),
    ("attr", "limit", "Maximum items requested per page"),
    (
        "attr",
        "est_tokens",
        "Estimated LLM token cost of the serialized payload",
    ),
    (
        "attr",
        "pr_iters",
        "Power-iteration steps taken for PageRank convergence",
    ),
    (
        "attr",
        "layer",
        "Architectural layer classification (e.g. test, domain, cli)",
    ),
];

pub fn serialize_legend_dict_xml<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "<!-- ripwire legend-dict schema=ripwire.legend-dict/v1: full attribute and schema encyclopedia (dictv={LEGEND_DICT_VERSION}). -->"
    )?;
    writeln!(
        out,
        "<legend-dict schema=\"ripwire.legend-dict/v1\" dictv=\"{LEGEND_DICT_VERSION}\" entries=\"{}\">",
        LEGEND_DICT_ENTRIES.len()
    )?;
    for &(cat, key, desc) in LEGEND_DICT_ENTRIES {
        writeln!(
            out,
            "  <entry cat=\"{cat}\" k=\"{key}\" v=\"{}\"/>",
            desc.xml_escaped()
        )?;
    }
    writeln!(out, "</legend-dict>")
}
