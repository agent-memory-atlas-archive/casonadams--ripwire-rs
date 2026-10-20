pub use super::arch_xml::*;
pub use super::legend::*;
pub use super::workspace_xml::*;

use crate::serialize::JsonValueExt;
use crate::serialize::escape::{XmlEscapeExt, escape_xml_str};
use serde_json::Value;
use std::io::{self, Write};
fn write_for_sig_metrics<W: Write>(out: &mut W, it: &Value) -> io::Result<()> {
    let ccx = it.u64_or("ccx", 0);
    let in_deg = it.u64_or("in", 0);
    let churn = it.u64_or("churn", 1);
    let amp = it.u64_or("amp", 1);
    if ccx > 0 {
        write!(out, " ccx=\"{ccx}\"")?;
    }
    if in_deg > 0 {
        write!(out, " in=\"{in_deg}\"")?;
    }
    if churn > 1 {
        write!(out, " churn=\"{churn}\"")?;
    }
    if amp > 1 {
        write!(out, " amp=\"{amp}\"")?;
    }
    if it.u64_or("tested", 0) > 0 {
        out.write_all(b" tested=\"1\"")?;
    }
    Ok(())
}

/// Serialize `for` task-lens ranking result to Ripwire XML.
fn write_for_sig_row<W: Write>(out: &mut W, idx: usize, it: &Value) -> io::Result<()> {
    use crate::serialize::PathLayerExt;
    let raw_name = it.str_or("name", "");
    let path = it.str_or("path", "");
    let line = it.u64_or("line", 0);
    let end_line = it.u64_or("end_line", line);
    let r = idx + 1;
    write!(out, "<d l=\"{line}\"")?;
    if end_line > line {
        write!(out, " e=\"{end_line}\"")?;
    }
    write!(
        out,
        " n=\"{}\" p=\"{}\"",
        raw_name.xml_escaped(),
        path.xml_escaped(),
    )?;
    path.write_layer_attr(out)?;
    write_for_sig_metrics(out, it)?;
    write!(
        out,
        " r=\"{r}\">{}</d>",
        it.str_or("sig", raw_name).xml_escaped()
    )
}

fn write_for_inner_body<W: Write>(
    out: &mut W,
    val: &Value,
    task: &str,
    raw_task: &str,
) -> io::Result<()> {
    let mut bodies_to_show = Vec::new();
    let items = val["symbols"].as_array();
    let sig_cnt = items.map_or(0, Vec::len);
    if sig_cnt >= 25 {
        write!(
            out,
            "<sigs shown=\"{sig_cnt}\" total=\"{sig_cnt}\" capped=\"1\" next=\"--for={task} --limit=40\">"
        )?;
    } else {
        write!(out, "<sigs>")?;
    }
    if let Some(items) = items {
        for (idx, it) in items.iter().enumerate() {
            write_for_sig_row(out, idx, it)?;
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
    write_for_compose_section(out, val, raw_task)?;
    write_for_tree_and_docs(out, val)?;
    write_for_hops_section(out, val)?;
    let n = bodies_to_show.len();
    write!(out, "<bodies shown=\"{n}\" total=\"{n}\" capped=\"0\">")?;
    for (k, l, p, name, b) in bodies_to_show {
        write!(
            out,
            "<b t=\"{k}\" l=\"{l}\" p=\"{p}\" n=\"{name}\"><![CDATA[{b}]]></b>"
        )?;
    }
    writeln!(out, "</bodies>")
}

pub struct ForReport<'a> {
    pub val: &'a Value,
    pub inner: Vec<u8>,
}

impl crate::serialize::XmlReport for ForReport<'_> {
    crate::xml_report_meta!("ctx", "ripwire-rs.for/v1", root: false);
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        use crate::serialize::GitStampExt;
        let raw_task = self.val.str_or("task", "");
        let task = escape_xml_str(raw_task);
        let default_route = format!("name-exact({raw_task})");
        let route = escape_xml_str(self.val.str_or("route", &default_route));
        let raw_root = self.val.str_or("root", ".");
        let root = escape_xml_str(raw_root);
        let at_stamp = raw_root.git_at_or_head();
        let confidence = escape_xml_str(self.val.str_or("confidence", "high"));
        let margin_pct = self.val.u64_or("margin_pct", 0);
        let coverage = self.val.u64_or("coverage", 100);
        let bodies_cnt = self.val.u64_or("detail", 1).max(1);
        let computed_tokens = ((self.inner.len() + 180) / 4).max(1) as u64;
        let est_tokens = self.val.u64_or("est_tokens", computed_tokens);
        write!(
            out,
            " task=\"{task}\" route=\"{route}\" root=\"{root}\" confidence=\"{confidence}\" margin_pct=\"{margin_pct}\" coverage=\"{coverage}\" at=\"{at_stamp}\" bundle=\"auto\" bodies=\"{bodies_cnt}\" est_tokens=\"{est_tokens}\""
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        out.write_all(&self.inner)
    }
}

pub fn serialize_for_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    let raw_task = val.str_or("task", "");
    let task = escape_xml_str(raw_task);
    let mut inner = Vec::with_capacity(4096);
    write_for_inner_body(&mut inner, val, &task, raw_task)?;
    ForReport { val, inner }.render_envelope(out)
}

fn for_section_requested(val: &Value, name: &str) -> bool {
    val["sections"]
        .as_array()
        .is_some_and(|s| s.iter().any(|v| v.as_str() == Some(name)))
}
fn write_for_tree_and_docs<W: Write>(out: &mut W, val: &Value) -> io::Result<()> {
    if let Some(tree) = val["tree"].as_array().filter(|a| !a.is_empty()) {
        for row in tree {
            let p = row.str_or("path", "").xml_escaped();
            let s = row.str_or("sym", "").xml_escaped();
            write!(out, "<t p=\"{p}\" sym=\"{s}\"/>")?;
        }
    }
    if let Some(docs) = val["docs"].as_array().filter(|a| !a.is_empty()) {
        for doc in docs {
            let p = doc.str_or("path", "").xml_escaped();
            let m = doc.u64_or("mentions", 1);
            write!(out, "<doc p=\"{p}\" mentions=\"{m}\"/>")?;
        }
    }
    Ok(())
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

fn write_for_compose_section<W: Write>(out: &mut W, val: &Value, raw_task: &str) -> io::Result<()> {
    let Some(fields) = val["compose"].as_array().filter(|a| !a.is_empty()) else {
        return Ok(());
    };
    if !for_section_requested(val, "compose") {
        let replay = format!(
            "--for='{}' --sections=compose",
            raw_task.replace('\'', "'\\''")
        );
        return write!(
            out,
            "<compose total=\"{}\" shown=\"0\" capped=\"1\" next=\"{}\"/>",
            fields.len(),
            replay.xml_escaped()
        );
    }
    write!(out, "<compose>")?;
    for f in fields {
        write!(
            out,
            "<field name=\"{}\" owner=\"{}\" rel=\"{}\"/>",
            f.str_or("name", "").xml_escaped(),
            f.str_or("owner", "").xml_escaped(),
            f.str_or("rel", "uses").xml_escaped()
        )?;
    }
    write!(out, "</compose>")
}

fn write_for_hops_section<W: Write>(out: &mut W, val: &Value) -> io::Result<()> {
    let Some(hops) = val["hops"].as_array().filter(|a| !a.is_empty()) else {
        return Ok(());
    };
    let n = hops.len();
    write!(out, "<hops shown=\"{n}\" total=\"{n}\" capped=\"0\">")?;
    for h in hops {
        let l = h.u64_or("line", 0);
        let p = h.str_or("path", "").xml_escaped();
        let name = h.str_or("name", "").xml_escaped();
        let total = h.u64_or("total", 0);
        write!(
            out,
            "<h l=\"{l}\" p=\"{p}\" n=\"{name}\"><calls total=\"{total}\">"
        )?;
        if let Some(calls) = h["calls"].as_array() {
            for c in calls {
                write!(
                    out,
                    "<c n=\"{}\" l=\"{}\"/>",
                    c.str_or("name", "").xml_escaped(),
                    c.u64_or("line", 0)
                )?;
            }
        }
        write!(out, "</calls></h>")?;
    }
    write!(out, "</hops>")
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

pub struct GrepReport<'a> {
    pub val: &'a Value,
}

impl crate::serialize::XmlReport for GrepReport<'_> {
    crate::xml_report_meta!("grep", "ripwire-rs.grep/v1", root: false);
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let pat = escape_xml_str(self.val["pattern"].as_str().unwrap_or(""));
        let hits = self.val["hits"].as_array();
        let count = hits.map_or(0, Vec::len);
        let cap = self.val.u64_or("limit", 100).max(1) as usize;
        let shown = count.min(cap);
        let capped = u8::from(shown < count);
        let (files_set, _, first_loc) = collect_grep_summary(hits);
        let f_cnt = files_set.len();
        write!(
            out,
            " pattern=\"{pat}\" root=\".\" files=\"{f_cnt}\" hits=\"{count}\" shown=\"{shown}\" capped=\"{capped}\" hits_capped=\"{capped}\" suppressed_comment=\"0\" suppressed_string=\"0\" tier_parsed=\"{f_cnt}\" tier_unclassified=\"0\" corpus_pruned_dirs=\"0\" unindexed_hits=\"0\" unindexed_files_scanned=\"0\" next=\"--at={}:{}\"",
            escape_xml_str(&first_loc.0),
            first_loc.1
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let hits = self.val["hits"].as_array();
        let count = hits.map_or(0, Vec::len);
        let cap = self.val.u64_or("limit", 100).max(1) as usize;
        let shown = count.min(cap);
        let (_, enc_set, _) = collect_grep_summary(hits);
        if let Some(items) = hits {
            for h in items.iter().take(shown) {
                write_grep_hit_row(out, h)?;
            }
        }
        write_grep_enc_rows(out, &enc_set)
    }
}

pub fn serialize_grep_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    GrepReport { val }.render_envelope(out)
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

pub struct ConnectReport<'a> {
    pub val: &'a Value,
}

impl crate::serialize::XmlReport for ConnectReport<'_> {
    crate::xml_report_meta!("connect", "ripwire-rs.connect/v1", root: false);
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let count = self.val["symbols"].as_array().map_or(0, Vec::len);
        let root = escape_xml_str(self.val["root"].as_str().unwrap_or("."));
        let t_cnt = self.val["terminals"].as_array().map_or(count, Vec::len);
        let s_cnt = self.val["steiner"].as_array().map_or(0, Vec::len);
        let e_cnt = self
            .val
            .array_any(&["edges", "connections"])
            .map_or(0, Vec::len);
        let nodes = t_cnt + s_cnt;
        write!(
            out,
            " root=\"{root}\" symbols=\"{count}\" terminals=\"{t_cnt}\" nodes=\"{nodes}\" edges=\"{e_cnt}\" radius=\"6\""
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        if let Some(terms) = self.val["terminals"].as_array() {
            for t in terms {
                write_connect_node_tag(out, "t", t)?;
            }
        }
        if let Some(joins) = self.val["steiner"].as_array() {
            for s in joins {
                write_connect_node_tag(out, "s", s)?;
            }
        }
        if let Some(edge_list) = self.val.array_any(&["edges", "connections"]) {
            for e in edge_list {
                let f = e["from"].as_str().unwrap_or("").xml_escaped();
                let t = e["to"].as_str().unwrap_or("").xml_escaped();
                writeln!(out, "  <e f=\"{f}\" t=\"{t}\"/>")?;
            }
        }
        if let Some(conn_list) = self.val.array_any(&["connections", "edges"]) {
            for c in conn_list {
                let f = c["from"].as_str().unwrap_or("").xml_escaped();
                let t = c["to"].as_str().unwrap_or("").xml_escaped();
                writeln!(out, "  <edge from=\"{f}\" to=\"{t}\"/>")?;
            }
        }
        Ok(())
    }
}

pub fn serialize_connect_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    ConnectReport { val }.render_envelope(out)
}

fn write_path_step<W: Write>(step: &Value, out: &mut W) -> io::Result<()> {
    if let Some(s) = step.as_str() {
        return writeln!(out, "  <s t=\"fn\" n=\"{}\"/>", s.xml_escaped());
    }
    let kind = step.str_or("kind", "fn").xml_escaped();
    let name = step.str_or("name", "").xml_escaped();
    let raw_path = step.str_or("path", "");
    let line = step.u64_or("line", 0);
    if !raw_path.is_empty() && line > 0 {
        let path = raw_path.xml_escaped();
        writeln!(out, "  <s t=\"{kind}\" n=\"{name}\" p=\"{path}:{line}\"/>")
    } else {
        writeln!(out, "  <s t=\"{kind}\" n=\"{name}\"/>")
    }
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
        "<path schema=\"ripwire-rs.path/v1\" from=\"{from}\" to=\"{to}\" from_defs=\"{from_defs}\" to_defs=\"{to_defs}\" reachable=\"{reachable}\" hops=\"{hops}\""
    )?;
    if !is_reach {
        write!(
            out,
            " hint=\"no directed call path — try --connect={from},{to} (undirected: finds a shared caller), or --uses/--impact for non-call references\""
        )?;
    }
    writeln!(out, ">")?;
    if let Some(steps) = val
        .get("steps")
        .or_else(|| val.get("path"))
        .and_then(Value::as_array)
    {
        for step in steps {
            write_path_step(step, out)?;
        }
    }
    writeln!(out, "</path>")
}
/// Serialize `--exemplar` to Ripwire XML.
fn write_exemplar_calls_xml<W: Write>(out: &mut W, ex: &Value) -> io::Result<()> {
    let Some(calls) = ex["calls"].as_array().filter(|a| !a.is_empty()) else {
        return Ok(());
    };
    write!(out, "<calls total=\"{}\">", calls.len())?;
    for c in calls {
        let cn = c.str_or("name", "").xml_escaped();
        let cl = c.u64_or("line", 1);
        let sig = c.str_or("sig", "").xml_escaped();
        write!(out, "<c n=\"{cn}\" l=\"{cl}\">{sig}</c>")?;
    }
    out.write_all(b"</calls>")
}

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

    write!(
        out,
        "<!-- ripwire-rs exemplar schema=ripwire-rs.exemplar/v1: the best-in-class instance of kind= for the task. -->\n<exemplar schema=\"ripwire-rs.exemplar/v1\"{cand_attr} kind=\"{k}\" n=\"{n}\" p=\"{p}:{l}\" in=\"{c}\" ccx=\"{cx}\" root=\"{root}\" tested=\"{tested}\"><bodies shown=\"1\" total=\"1\" capped=\"0\"><b t=\"{k}\" l=\"{l}\" p=\"{p}\" n=\"{n}\"><![CDATA[{b}]]>",
    )?;
    write_exemplar_calls_xml(out, ex)?;
    writeln!(out, "</b></bodies></exemplar>")
}

/// Serialize `--lego` to Ripwire XML.
pub fn serialize_lego_xml<W: Write>(root: &str, val: &Value, out: &mut W) -> io::Result<()> {
    let iface = val.str_or("interface", "");
    let empty = Vec::new();
    let impls = val["implementors"].as_array().unwrap_or(&empty);
    let methods = val["methods"].as_array().unwrap_or(&empty);
    let defs = val.u64_or("defs", 1).max(1);
    let p_attr = val
        .opt_str("path")
        .filter(|p| !p.is_empty())
        .map_or_else(String::new, |p| format!(" p=\"{}\"", p.xml_escaped()));
    let m_attr = if methods.is_empty() {
        String::new()
    } else {
        format!(" methods=\"{}\"", methods.len())
    };
    let lego_attrs = val.xml_opt_gauge_attrs();
    writeln!(
        out,
        "<ctx schema=\"ripwire-rs.lego/v1\" root=\"{root}\"><lego{lego_attrs}><iface n=\"{iface}\"{p_attr}{m_attr} defs=\"{defs}\" implementors=\"{}\">",
        impls.len()
    )?;
    for m in methods {
        let mn = m.str_or("name", "").xml_escaped();
        let ml = m.u64_or("line", 0);
        let mp = m.u64_or("params", 0);
        writeln!(out, "  <m n=\"{mn}\" l=\"{ml}\" params=\"{mp}\"/>")?;
    }
    for it in impls {
        let n = it.str_or("name", "");
        let p = it.str_or("path", "");
        let l = it.u64_or("line", 0);
        if l > 0 {
            writeln!(out, "  <impl n=\"{n}\" p=\"{p}:{l}\"/>")?;
        } else {
            writeln!(out, "  <impl n=\"{n}\" p=\"{p}\"/>")?;
        }
    }
    writeln!(out, "</iface></lego></ctx>")
}

fn push_recall_sections_note(buf: &mut String, s: &Value, whole_bytes: u64) {
    let selected = s.u64_or("selected", 0);
    let total = s.u64_or("total", 0);
    let lines = s.str_or("lines", "");
    let dropped = s.u64_or("dropped_by_budget", 0);
    if dropped == 0 {
        buf.push_str(&format!(
            "  [sections: {selected} of {total}, section-granular; whole doc {whole_bytes} B; lines=\"{lines}\"]"
        ));
    } else {
        let matched = s.u64_or("matched", 0);
        buf.push_str(&format!(
            "  [sections: {selected} of {matched} selected ({total} in doc), section-granular; whole doc {whole_bytes} B; lines=\"{lines}\"; dropped_by_budget={dropped}]"
        ));
    }
}

fn push_recall_doc(buf: &mut String, d: &Value) {
    buf.push_str(&format!(
        "━━ {}  (relevance {:.3}) ━━",
        d.str_or("path", ""),
        d.f64_any(&["score"], 0.0)
    ));
    if d.bool_or("generated", false) {
        buf.push_str("  [generated_demoted: marker]");
    }
    if let Some(s) = d.get("sections") {
        push_recall_sections_note(buf, s, d.u64_or("whole_bytes", 0));
    }
    if let Some(t) = d.get("truncated") {
        buf.push_str(&format!(
            "  [truncated: {} of {} bytes]",
            t.u64_or("shown_bytes", 0),
            t.u64_or("of_bytes", 0)
        ));
    }
    buf.push('\n');
    buf.push_str(d.str_or("content", ""));
    buf.push_str("\n\n");
}

fn push_recall_footer(buf: &mut String, val: &Value) {
    let relevant = val.u64_or("relevant", 0);
    if relevant == 0 {
        buf.push_str("(no relevant documents — try different terms)\n");
        return;
    }
    let by_k = val.u64_or("omitted_top_k", 0);
    let by_budget = val.u64_or("omitted_budget", 0);
    let mut parts = Vec::new();
    if by_k > 0 {
        let default_k = val.u64_or("default_top_k", 8);
        parts.push(format!(
            "raise --top-k/top_k (default {default_k}) for {by_k} more"
        ));
    }
    if by_budget > 0 {
        let budget = val.u64_or("budget_bytes", 0);
        parts.push(format!(
            "raise --max-tokens/budget_tokens or narrow the query for {by_budget} more (~{budget}-byte budget)"
        ));
    }
    if !parts.is_empty() {
        buf.push_str(&format!(
            "(capped: {} of {relevant} relevant document files omitted — {})\n",
            by_k + by_budget,
            parts.join("; ")
        ));
    }
}

fn recall_header(query: &str, val: &Value, shown: usize, est_tokens: u64) -> String {
    let relevant = val.u64_or("relevant", 0);
    let mut h = format!(
        "ripwire-rs recall — \"{query}\" — {relevant} relevant of {} document files, best-first — total={relevant} shown={shown} capped={}",
        val.u64_or("documents", 0),
        u8::from(val.bool_or("capped", false))
    );
    for key in ["truncated", "generated_demoted"] {
        let n = val.u64_or(key, 0);
        if n > 0 {
            h.push_str(&format!(" {key}={n}"));
        }
    }
    let max_tokens = val.u64_or("max_tokens", 0);
    if est_tokens > max_tokens {
        h.push_str(" over_ceiling=1");
    }
    h.push_str(&format!(" max_tokens={max_tokens}"));
    if let Some(share) = val.get("share_bytes").and_then(Value::as_u64) {
        h.push_str(&format!(" share_bytes={share}"));
    }
    h.push_str(&format!(" est_tokens={est_tokens}\n\n"));
    h
}

/// Serialize `--recall` as plain text (upstream's rendering): header, ranked doc blocks, cap footer.
pub fn serialize_recall_text<W: Write>(query: &str, val: &Value, out: &mut W) -> io::Result<()> {
    let empty = Vec::new();
    let docs = val.array_any(&["docs"]).unwrap_or(&empty);
    let mut rest = String::new();
    for d in docs {
        push_recall_doc(&mut rest, d);
    }
    push_recall_footer(&mut rest, val);
    let mut est_tokens = 0u64;
    let mut header = recall_header(query, val, docs.len(), est_tokens);
    for _ in 0..4 {
        let total_bytes = (header.len() + rest.len()) as u64;
        let next = (total_bytes * 2).div_ceil(5);
        if next == est_tokens {
            break;
        }
        est_tokens = next;
        header = recall_header(query, val, docs.len(), est_tokens);
    }
    out.write_all(header.as_bytes())?;
    out.write_all(rest.as_bytes())
}

/// Serialize `--from-trace` to Ripwire XML.
pub fn serialize_from_trace_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    let empty = Vec::new();
    let suspects = val["suspects"].as_array().unwrap_or(&empty);
    writeln!(
        out,
        "<from-trace schema=\"ripwire-rs.from-trace/v1\" count=\"{}\">",
        suspects.len()
    )?;
    for s in suspects {
        let n = s.str_or("name", "");
        let p = s.str_any(&["path", "file"], "");
        let l = s.u64_or("line", 0);
        writeln!(out, "  <suspect n=\"{n}\" p=\"{p}\" l=\"{l}\"/>")?;
    }
    writeln!(out, "</from-trace>")
}
