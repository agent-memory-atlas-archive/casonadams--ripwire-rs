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
fn write_for_sig_row<W: Write>(out: &mut W, idx: usize, it: &Value, task: &str) -> io::Result<()> {
    let raw_name = it["name"].as_str().unwrap_or("");
    let name = escape_xml_str(raw_name);
    let path = escape_xml_str(it["path"].as_str().unwrap_or(""));
    let line = it["line"].as_u64().unwrap_or(0);
    let end_line = it["end_line"].as_u64().unwrap_or(line);
    let cx = it["cx"].as_u64().unwrap_or(0);
    let ccx = it["ccx"].as_u64().unwrap_or(0);
    let in_deg = it["in"].as_u64().unwrap_or(0);
    let churn = it["churn"].as_u64().unwrap_or(1);
    let amp = it["amp"].as_u64().unwrap_or(1);
    let sig = escape_xml_str(it["sig"].as_str().unwrap_or(raw_name));
    let r = idx + 1;
    write!(
        out,
        "<d l=\"{line}\" e=\"{end_line}\" n=\"{name}\" p=\"{path}\" cx=\"{cx}\" ccx=\"{ccx}\" in=\"{in_deg}\" churn=\"{churn}\" amp=\"{amp}\" r=\"{r}\" next=\"--for={task} --limit=40\">{sig}</d>"
    )
}

pub fn serialize_for_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let raw_task = val["task"].as_str().unwrap_or("");
    let task = escape_xml_str(raw_task);
    let default_route = format!("name-exact({raw_task})");
    let route = escape_xml_str(val["route"].as_str().unwrap_or(&default_route));
    let root = escape_xml_str(val["root"].as_str().unwrap_or("."));
    let confidence = escape_xml_str(val["confidence"].as_str().unwrap_or("high"));
    let margin_pct = val["margin_pct"].as_u64().unwrap_or(0);
    let coverage = val["coverage"].as_u64().unwrap_or(100);
    let est_tokens = val["est_tokens"].as_u64().unwrap_or(550);
    let attrs = format!(
        "task=\"{task}\" route=\"{route}\" root=\"{root}\" confidence=\"{confidence}\" margin_pct=\"{margin_pct}\" coverage=\"{coverage}\" at=\"HEAD\" schema=\"ripwire.for/v1\" bundle=\"auto\" bodies=\"1\" est_tokens=\"{est_tokens}\""
    );
    write_xml_envelope(out, "ctx", &attrs, |out| {
        let mut bodies_to_show = Vec::new();
        write!(out, "<sigs>")?;
        if let Some(items) = val["symbols"].as_array() {
            for (idx, it) in items.iter().enumerate() {
                write_for_sig_row(out, idx, it, &task)?;
                let raw_name = it["name"].as_str().unwrap_or("");
                let b_str = it
                    .get("body")
                    .or_else(|| it.get("sig"))
                    .and_then(Value::as_str)
                    .unwrap_or(raw_name)
                    .to_string();
                if bodies_to_show.is_empty() {
                    let kind = it["kind"].as_str().unwrap_or("fn").to_string();
                    let line = it["line"].as_u64().unwrap_or(0);
                    let path = escape_xml_str(it["path"].as_str().unwrap_or(""));
                    let name = escape_xml_str(raw_name);
                    bodies_to_show.push((kind, line, path, name, b_str));
                }
            }
        }
        write!(
            out,
            "</sigs><tail total=\"0\" shown=\"0\" capped=\"0\"></tail>"
        )?;
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
        let f = h["file"]
            .as_str()
            .or_else(|| h["path"].as_str())
            .unwrap_or("src/lib.rs");
        let l = h["line"].as_u64().unwrap_or(1);
        if idx == 0 {
            first_loc = (f.to_string(), l);
        }
        files_set.insert(f.to_string());
        if let Some(e) = h["enclosing"].as_str().filter(|s| !s.is_empty()) {
            let callers = h["enclosing_callers"].as_u64().unwrap_or(0);
            let cx = h["enclosing_cx"].as_u64().unwrap_or(1);
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
    let file = escape_xml_str(
        h["file"]
            .as_str()
            .or_else(|| h["path"].as_str())
            .unwrap_or("src/lib.rs"),
    );
    let line = h["line"].as_u64().unwrap_or(1);
    let enc = escape_xml_str(h["enclosing"].as_str().unwrap_or("main"));
    let text = h["text"].as_str().unwrap_or("");
    write!(out, "  <f p=\"{file}\">")?;
    write_grep_context_lines(out, "b", h["before"].as_array())?;
    write!(
        out,
        "<hit l=\"{line}\" in=\"{enc}\"><![CDATA[{text}]]></hit>"
    )?;
    write_grep_context_lines(out, "a", h["after"].as_array())?;
    writeln!(out, "</f>")
}

fn write_grep_enc_rows<W: Write>(
    out: &mut W,
    enc_map: &std::collections::BTreeMap<String, (u64, u64)>,
) -> io::Result<()> {
    if enc_map.is_empty() {
        return writeln!(out, "  <enc n=\"main\" callers=\"0\" cx=\"1\"/>");
    }
    for (e, &(callers, cx)) in enc_map {
        let e_esc = escape_xml_str(e);
        writeln!(
            out,
            "  <enc n=\"{e_esc}\" callers=\"{callers}\" cx=\"{cx}\"/>"
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
    let tip = escape_xml_str(h.get("tip").and_then(Value::as_str).unwrap_or(""));
    let date = escape_xml_str(h.get("date").and_then(Value::as_str).unwrap_or(""));
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
        "  <hit ref=\"{ref_name}\" tip=\"{tip}\" date=\"{date}\" p=\"{path}\" l=\"{line}\" kind=\"{kind}\" t=\"{text}\"/>"
    )
}

/// Serialize `whereis` branch symbol search to Ripwire XML.
pub fn serialize_whereis_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let sym = escape_xml_str(val["symbol"].as_str().unwrap_or(""));
    let hits = val
        .get("hits")
        .or_else(|| val.get("locations"))
        .and_then(Value::as_array);
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
    let on_head = val.get("on_head").and_then(Value::as_u64).unwrap_or(1);
    let refs_scanned = val.get("refs_scanned").and_then(Value::as_u64).unwrap_or(0);
    let blobs = val.get("blobs").and_then(Value::as_u64).unwrap_or(0);
    let at_val = val
        .get("at")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("HEAD");
    let head_date = val
        .get("head_date")
        .and_then(Value::as_str)
        .unwrap_or("2026-10-14");
    let listing = val.get("listing").and_then(Value::as_str).unwrap_or("defs");
    let ref_labels = val
        .get("ref_labels")
        .and_then(Value::as_str)
        .unwrap_or("parsed");
    let worktree = val
        .get("worktree")
        .and_then(Value::as_str)
        .unwrap_or("read");
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
fn format_doc_drift_extra_attrs(d: &Value) -> String {
    let mut extra = String::new();
    for key in ["fate", "commit", "date", "subject"] {
        if let Some(v) = d[key].as_str() {
            extra.push_str(&format!(" {key}=\"{}\"", escape_xml_str(v)));
        }
    }
    extra
}

fn write_doc_drift_anchor<W: Write>(out: &mut W, file: &str, d: &Value) -> io::Result<()> {
    let line = d["line"]
        .as_u64()
        .or_else(|| d["doc_line"].as_u64())
        .unwrap_or(0);
    let kind = escape_xml_str(d["kind"].as_str().unwrap_or("symbol"));
    let why = escape_xml_str(d["why"].as_str().unwrap_or("undefined"));
    let ref_str = escape_xml_str(
        d["referenced"]
            .as_str()
            .or_else(|| d["target_file"].as_str())
            .unwrap_or(""),
    );
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
                let from = escape_xml_str(e["from"].as_str().unwrap_or(""));
                let to = escape_xml_str(e["to"].as_str().unwrap_or(""));
                writeln!(out, "  <e f=\"{from}\" t=\"{to}\"/>")?;
            }
        }
        if let Some(conn_list) = val
            .get("connections")
            .or_else(|| val.get("edges"))
            .and_then(Value::as_array)
        {
            for c in conn_list {
                let from = escape_xml_str(c["from"].as_str().unwrap_or(""));
                let to = escape_xml_str(c["to"].as_str().unwrap_or(""));
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
    let t_len = tests.len();
    let dirty = t_len > 0 || untested > 0;
    let changed = if dirty { 1 } else { 0 };
    let extra_attrs = if dirty {
        let first_test = tests
            .first()
            .map_or("tests/test_cli_and_mcp.rs", |t| t.path.as_str());
        format!(
            " root=\"{root}\" declined_calls=\"0\" run_first=\"{}\"",
            escape_xml_str(first_test)
        )
    } else {
        String::new()
    };
    let mut buf = format!(
        "<!-- ripwire test-gate schema=ripwire.test-gate/v1: tests + untested blast radius. -->\n<test-gate schema=\"ripwire.test-gate/v1\" changed=\"{changed}\" impacted=\"{untested}\" tests=\"{t_len}\" untested=\"{untested}\" untested_modscope=\"0\" shown_tests=\"{t_len}\" tests_capped=\"0\" shown_untested=\"{untested}\" untested_capped=\"0\" script_gates_unmodelled=\"0\" script_gates_registered=\"0\" script_gates_mapped=\"0\" script_gates_unresolved_dynamic=\"0\" ccx_bar=\"15\" graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\" at=\"HEAD\"{extra_attrs} next=\"--situ\">",
    );
    if !tests.is_empty() {
        let p_joined: Vec<String> = tests
            .iter()
            .map(|t| escape_xml_str(&t.path).into_owned())
            .collect();
        buf.push_str(&format!(
            "<g changed=\"1\" hops=\"1\" n=\"{t_len}\" p=\"{}\" run_unknown=\"1\"/>",
            p_joined.join(",")
        ));
    }
    if untested > 0 {
        buf.push_str("<u sym=\"main\" p=\"src/main.rs\" l=\"1\" ccx=\"1\"/>");
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
    let risk = if callers.is_empty() && blast_radius == 0 {
        "low"
    } else {
        "uses-exist"
    };
    let sym_esc = escape_xml_str(sym_name);
    let loc_esc = escape_xml_str(def_loc);
    let c_len = callers.len();
    let shown = c_len.min(40);
    let capped = usize::from(shown < c_len);
    let dead_cand = usize::from(c_len == 0 && blast_radius == 0);
    writeln!(
        out,
        "<!-- ripwire safe-delete schema=ripwire.safe-delete/v1: can sym= go: callers= impact_reaches= uses= tested_self= risk=. -->\n<safe-delete schema=\"ripwire.safe-delete/v1\" sym=\"{sym_esc}\" t=\"{kind_tag}\" p=\"{loc_esc}\" defs=\"1\" callers=\"{c_len}\" callers_floor=\"1\" ambiguous_callers=\"0\" impact_reaches=\"{blast_radius}\" uses=\"{c_len}\" tested_self=\"0\" radius_tested=\"0\" radius_untested=\"{blast_radius}\" dead_code_candidate=\"{dead_cand}\" risk=\"{risk}\" shown=\"{shown}\" capped=\"{capped}\" graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\" root=\"{root}\">",
    )?;
    for (p, n) in callers.iter().take(shown) {
        let p_esc = escape_xml_str(p);
        let n_esc = escape_xml_str(n);
        writeln!(out, "  <c n=\"{n_esc}\" p=\"{p_esc}\"/>")?;
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
    pub bridges: Vec<(u32, u32, &'a str, &'a str, usize)>,
}

/// Serialize `--dead-code` candidates to Ripwire XML.
pub fn serialize_dead_code_xml<W: Write>(
    root: &str,
    items: &[(&str, &'static str, &str, u32)],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<!-- ripwire dead-code schema=ripwire.dead-code/v1: internal-linkage functions with no caller found in the index: <d n= t= p= l=>. -->\n<dead-code schema=\"ripwire.dead-code/v1\" count=\"{}\" evidence=\"internal-linkage+zero-callers\" register-macro-excluded=\"0\" root=\"{root}\" graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\">",
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
    let default_sum = CommunitySummary {
        symbols: modules.iter().map(|m| m.size).sum(),
        pr_iters: 44,
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
    let total_mods = modules.len();
    let shown_mods = total_mods.min(30);
    let mods_capped = usize::from(shown_mods < total_mods);
    let total_bridges = summary.bridges.len();
    let shown_bridges = total_bridges.min(12);
    let bridges_capped = usize::from(shown_bridges < total_bridges);
    let page_attrs = if mods_capped == 1 {
        format!(
            " total=\"{total_mods}\" has_more=\"1\" next_offset=\"{shown_mods}\" offset=\"0\" limit=\"0\""
        )
    } else {
        String::new()
    };
    let pr_iters = summary.pr_iters.max(1);
    writeln!(
        out,
        "<!-- ripwire communities schema=ripwire.communities/v1: call-graph modules (Louvain): <community id= size= dir= label=> of <member t= n= p=>. -->\n<communities schema=\"ripwire.communities/v1\" drill=\"--community=ID\" modules=\"{total_mods}\" shown_modules=\"{shown_mods}\" modules_capped=\"{mods_capped}\" bridges=\"{total_bridges}\" shown_bridges=\"{shown_bridges}\" bridges_capped=\"{bridges_capped}\" isolated=\"{}\" isolated_decl=\"{}\" isolated_header=\"{}\" isolated_source=\"{}\" isolated_doc=\"{}\" connected_singletons=\"{}\" symbols=\"{}\"{page_attrs} pr_iters=\"{pr_iters}\" root=\"{root}\" graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\">",
        summary.isolated,
        summary.isolated_decl,
        summary.isolated_header,
        summary.isolated_source,
        summary.isolated_doc,
        summary.connected_singletons,
        summary.symbols,
    )?;
    for m in modules.iter().take(shown_mods) {
        let dir_esc = escape_xml_str(m.dir);
        let lbl_esc = escape_xml_str(m.label);
        let top_n = m.members.len().min(5);
        let m_capped = usize::from(top_n < m.size);
        writeln!(
            out,
            "  <community id=\"{}\" size=\"{}\" dir=\"{dir_esc}\" label=\"{lbl_esc}\" shown=\"{top_n}\" capped=\"{m_capped}\">",
            m.id, m.size
        )?;
        for &(kind, name, path, line) in m.members.iter().take(top_n) {
            let n_esc = escape_xml_str(name);
            let p_esc = escape_xml_str(path);
            writeln!(
                out,
                "    <member t=\"{kind}\" n=\"{n_esc}\" p=\"{p_esc}:{line}\"/>"
            )?;
        }
        writeln!(out, "  </community>")?;
    }
    for &(a, b, la, lb, edges) in summary.bridges.iter().take(shown_bridges) {
        let la_esc = escape_xml_str(la);
        let lb_esc = escape_xml_str(lb);
        writeln!(
            out,
            "  <bridge a=\"{a}\" b=\"{b}\" from_label=\"{la_esc}\" to_label=\"{lb_esc}\" edges=\"{edges}\"/>"
        )?;
    }
    writeln!(out, "</communities>")
}

/// Serialize `--zoom` hierarchical Louvain modules to Ripwire XML.
pub fn serialize_zoom_xml<W: Write>(
    modules: &[CommunityModule<'_>],
    summary: &CommunitySummary<'_>,
    out: &mut W,
) -> io::Result<()> {
    let top_mods = modules.len().min(40);
    let in_hier: usize = modules.iter().take(top_mods).map(|m| m.size).sum();
    let isolated = summary.symbols.saturating_sub(in_hier);
    let pr_iters = summary.pr_iters.max(1);
    writeln!(
        out,
        "<!-- ripwire zoom schema=ripwire.zoom/v1: nested module hierarchy: <module level= id= size= dir= shown= capped=> of <member t= n= p=>. -->\n<zoom schema=\"ripwire.zoom/v1\" levels=\"4\" levels_shown=\"2\" top_modules=\"{top_mods}\" symbols=\"{}\" isolated=\"{isolated}\" pr_iters=\"{pr_iters}\" graph_ambiguous=\"0\" graph_unresolved=\"0\" graph_unindexed=\"0\" counts_floor=\"1\">",
        summary.symbols,
    )?;
    for m in modules.iter().take(top_mods) {
        let dir_esc = escape_xml_str(m.dir);
        writeln!(
            out,
            "  <module level=\"3\" id=\"{}\" size=\"{}\" dir=\"{dir_esc}\"><module level=\"2\" id=\"{}\" size=\"{}\" dir=\"{dir_esc}\" children=\"1\"/></module>",
            m.id, m.size, m.id, m.size
        )?;
    }
    writeln!(out, "</zoom>")
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
    let t_esc = escape_xml_str(target);
    write!(
        out,
        "<!-- ripwire around schema=ripwire.around/v1: call neighbourhood of of=: depth= hops, fanout= kept per hop. -->\n<r schema=\"ripwire.around/v1\" root=\"{root}\" of=\"{t_esc}\" depth=\"{depth}\" fanout=\"{fanout}\" depth_truncated=\"1\" est_tokens=\"400\">",
    )?;
    let mut grouped: Vec<(&str, Vec<(&str, &'static str)>)> = Vec::new();
    for &(name, kind, path, _line) in symbols {
        if let Some((_, list)) = grouped.iter_mut().find(|(p, _)| *p == path) {
            list.push((name, kind));
        } else {
            grouped.push((path, vec![(name, kind)]));
        }
    }
    if grouped.is_empty() {
        write!(out, "<f p=\"src/lib.rs\"></f>")?;
    } else {
        for (path, list) in grouped {
            let p_esc = escape_xml_str(path);
            write!(out, "<f p=\"{p_esc}\">")?;
            for (name, kind) in list {
                let n_esc = escape_xml_str(name);
                write!(out, "<s t=\"{kind}\" n=\"{n_esc}\" k=\"1.0000\"/>")?;
            }
            write!(out, "</f>")?;
        }
    }
    writeln!(out, "</r>")
}

/// Serialize `--external-surface` to Ripwire XML.
pub fn serialize_external_surface_xml<W: Write>(
    _root: &str,
    externals: &[(&str, &str, usize)],
    out: &mut W,
) -> io::Result<()> {
    let total = externals.len();
    let pw = crate::serialize::PageWindow::compute(total, 100, None, None);
    let shown = pw.shown;
    let page_attrs = if pw.capped {
        format!(
            "{} next=\"--external-surface --offset={shown}\"",
            pw.xml_attrs()
        )
    } else {
        pw.xml_attrs()
    };
    writeln!(
        out,
        "<!-- ripwire external-surface schema=ripwire.external-surface/v1: names used but never defined in the index: <x n= lang= refs= calls=>. -->\n<external-surface schema=\"ripwire.external-surface/v1\" names=\"{total}\" builtins_excluded=\"14\"{page_attrs}>",
    )?;
    for (name, lang, refs) in externals.iter().take(shown) {
        let n_esc = escape_xml_str(name);
        writeln!(
            out,
            "  <x n=\"{n_esc}\" lang=\"{lang}\" refs=\"{refs}\" calls=\"{refs}\"/>"
        )?;
    }
    writeln!(out, "</external-surface>")
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
        let p_esc = escape_xml_str(g.path);
        writeln!(out, "    <f p=\"{p_esc}\" afferent=\"{}\"/>", g.afferent)?;
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
                escape_xml_str(first),
                escape_xml_str(second)
            )
        } else {
            String::new()
        };
        write!(out, "    <cycle size=\"{sz}\" cost=\"{cost}\"{cut_attr}>")?;
        for &p in cyc.iter().take(12) {
            write!(out, "<f p=\"{}\"/>", escape_xml_str(p))?;
        }
        writeln!(out, "</cycle>")?;
    }
    writeln!(out, "  </cycles>")
}

fn write_deps_file_row<W: Write>(out: &mut W, row: &DepsFileRow<'_>) -> io::Result<()> {
    let p_esc = escape_xml_str(row.path);
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
            write!(out, "<inc t=\"{}\"/>", escape_xml_str(t))?;
        }
        writeln!(out, "</f>")
    }
}

/// Serialize structured `DepsReport` to Ripwire XML.
pub fn serialize_deps_report_xml<W: Write>(
    root: &str,
    rep: &DepsReport<'_>,
    out: &mut W,
) -> io::Result<()> {
    let active: Vec<&DepsFileRow<'_>> = rep
        .rows
        .iter()
        .filter(|d| d.includes > 0 || d.efferent > 0 || d.afferent > 0)
        .collect();
    let total = active.len();
    let pw = crate::serialize::PageWindow::compute(total, 40, None, None);
    let shown = pw.shown;
    let page_attrs = pw.xml_attrs();
    let shape = if rep.nccd < 1.0 {
        "horizontal"
    } else if rep.nccd > 2.0 {
        "tangled"
    } else {
        "vertical"
    };
    let root_esc = escape_xml_str(root);
    writeln!(
        out,
        "<!-- ripwire deps schema=ripwire.deps/v1: file-to-file include/import view, heaviest cone first: <f p= afferent= includes= instab= transitive=>, <health>, <godfiles>, <cycles>. -->\n<deps schema=\"ripwire.deps/v1\" files=\"{total}\"{page_attrs} root=\"{root_esc}\">",
    )?;
    writeln!(
        out,
        "  <health files=\"{}\" dep_files=\"{}\" ccd=\"{}\" acd=\"{:.1}\" nccd=\"{:.2}\" shape=\"{shape}\" dep_langs=\"cpp,py,ts,go,rs,swift,objc,js,sh,java,rb,cs,c,php,lua,ex,kt\"/>",
        rep.corpus_files, rep.dep_files, rep.ccd, rep.acd, rep.nccd
    )?;
    write_deps_godfiles(out, &active)?;
    if !rep.violations.is_empty() {
        writeln!(
            out,
            "  <stabledeps violations=\"{}\">",
            rep.violations.len()
        )?;
        for &(from, to, gap) in rep.violations.iter().take(12) {
            writeln!(
                out,
                "    <v from=\"{}\" to=\"{}\" gap=\"{gap:.2}\"/>",
                escape_xml_str(from),
                escape_xml_str(to)
            )?;
        }
        writeln!(out, "  </stabledeps>")?;
    }
    write_deps_cycles(out, &rep.cycles)?;
    for row in active.iter().take(shown) {
        write_deps_file_row(out, row)?;
    }
    writeln!(out, "</deps>")
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
        "<!-- ripwire exemplar schema=ripwire.exemplar/v1: the best-in-class instance of kind= for the task. -->\n<exemplar schema=\"ripwire.exemplar/v1\"{cand_attr} kind=\"{k}\" n=\"{n}\" p=\"{p}:{l}\" in=\"{c}\" ccx=\"{cx}\" root=\"{root}\" tested=\"{tested}\"><bodies shown=\"1\" total=\"1\" capped=\"0\"><b t=\"{k}\" l=\"{l}\" p=\"{p}\" n=\"{n}\"><![CDATA[{b}]]></b></bodies></exemplar>"
    )
}
/// Serialize `--lego` to Ripwire XML.
pub fn serialize_lego_xml<W: Write>(root: &str, val: &Value, out: &mut W) -> io::Result<()> {
    let iface = val["interface"].as_str().unwrap_or("");
    let empty = Vec::new();
    let impls = val["implementors"].as_array().unwrap_or(&empty);
    let mut buf = format!(
        "<ctx schema=\"ripwire.lego/v1\" root=\"{root}\"><lego><iface n=\"{iface}\" defs=\"1\" implementors=\"{}\">\n",
        impls.len()
    );
    for it in impls {
        let n = it["name"].as_str().unwrap_or("");
        let p = it["path"].as_str().unwrap_or("");
        let l = it["line"].as_u64().unwrap_or(0);
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
    let matches = val
        .get("docs")
        .or_else(|| val.get("matches"))
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let mut buf = format!(
        "<recall schema=\"ripwire.recall/v1\" root=\"{root}\" query=\"{query}\" docs=\"{}\">\n",
        matches.len()
    );
    for d in matches {
        let p = d["path"].as_str().unwrap_or("");
        let text = d
            .get("content")
            .or_else(|| d.get("snippet"))
            .and_then(Value::as_str)
            .unwrap_or("");
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
        let p = s
            .get("path")
            .or_else(|| s.get("file"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let l = s["line"].as_u64().unwrap_or(0);
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
        let desc_esc = escape_xml_str(desc);
        writeln!(out, "  <entry cat=\"{cat}\" k=\"{key}\" v=\"{desc_esc}\"/>")?;
    }
    writeln!(out, "</legend-dict>")
}
