use crate::serialize::JsonValueExt;
use crate::serialize::escape::{XmlEscapeExt, escape_xml_str};
use serde_json::Value;
use std::io::{self, Write};

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

fn emit_whereis_hit<W: Write>(out: &mut W, h: &Value, refs: usize) -> io::Result<()> {
    let ref_name = h.str_or("ref", "HEAD").xml_escaped();
    let mut stamp = String::new();
    for key in ["tip", "date"] {
        if let Some(v) = h.opt_str(key).filter(|v| !v.is_empty()) {
            stamp.push_str(&format!(" {key}=\"{}\"", v.xml_escaped()));
        }
    }
    let path = h.str_any(&["path", "file"], "").xml_escaped();
    let line = h.u64_or("line", 0);
    let kind = h.str_or("kind", "def");
    let refs_attr = if refs > 1 {
        format!(" refs=\"{refs}\"")
    } else {
        String::new()
    };
    let text = h.str_or("text", "").xml_escaped();
    writeln!(
        out,
        "  <hit ref=\"{ref_name}\"{stamp} p=\"{path}\" l=\"{line}\" kind=\"{kind}\"{refs_attr} t=\"{text}\"/>"
    )
}

/// Rows a `--whereis` listing prints: `(hit, refs holding the same line)`, plus the folded count.
struct WhereisRows<'a> {
    listed: Vec<(&'a Value, usize)>,
    folded: usize,
    ref_rows: usize,
}

fn whereis_rows<'a>(hits: &'a [Value], listing: &str) -> WhereisRows<'a> {
    let is_ref = |h: &Value| h.str_or("kind", "ref") == "ref";
    let ref_rows = hits.iter().filter(|h| is_ref(h)).count();
    if listing != "defs" {
        let listed = hits
            .iter()
            .filter(|h| listing == "all" || is_ref(h))
            .map(|h| (h, 1))
            .collect();
        return WhereisRows {
            listed,
            folded: 0,
            ref_rows,
        };
    }
    let mut listed: Vec<(&Value, usize)> = Vec::new();
    let mut first_by_line: std::collections::HashMap<(&str, &str), usize> =
        std::collections::HashMap::new();
    let mut folded = 0;
    for h in hits.iter().filter(|h| !is_ref(h)) {
        if h.str_or("ref", "HEAD") == "HEAD" {
            listed.push((h, 1));
            continue;
        }
        let key = (h.str_any(&["path", "file"], ""), h.str_or("text", ""));
        if let Some(&i) = first_by_line.get(&key) {
            listed[i].1 += 1;
            folded += 1;
        } else {
            first_by_line.insert(key, listed.len());
            listed.push((h, 1));
        }
    }
    WhereisRows {
        listed,
        folded,
        ref_rows,
    }
}

/// Serialize `whereis` branch symbol search to Ripwire XML.
pub struct WhereisReport<'a> {
    pub val: &'a Value,
}

impl WhereisReport<'_> {
    fn hits(&self) -> &[Value] {
        self.val
            .array_any(&["hits", "locations"])
            .map_or(&[], Vec::as_slice)
    }

    fn listing(&self) -> &str {
        self.val.str_or("listing", "defs")
    }
}

impl crate::serialize::XmlReport for WhereisReport<'_> {
    crate::xml_report_meta!("whereis", "ripwire-rs.whereis/v1", root: false);
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        use crate::serialize::PaginateSlice;
        let sym = escape_xml_str(self.val.str_or("symbol", ""));
        let hits = self.hits();
        let rows = whereis_rows(hits, self.listing());
        let (pw, _) = rows.listed.paginate(60, None, None);
        let folded_attr = if rows.folded > 0 {
            format!(" folded=\"{}\"", rows.folded)
        } else {
            String::new()
        };
        let head_date = self
            .val
            .opt_str("head_date")
            .filter(|d| !d.is_empty())
            .map_or_else(String::new, |d| format!(" head_date=\"{d}\""));
        write!(
            out,
            " sym=\"{sym}\" on-head=\"{}\" refs_scanned=\"{}\" blobs=\"{}\" hits=\"{}\" head_labels=\"index\"{} listing=\"{}\"{folded_attr} ref_labels=\"{}\"{head_date} at=\"{}\" worktree=\"{}\" complete=\"1\"",
            self.val.u64_or("on_head", 1),
            self.val.u64_or("refs_scanned", 0),
            self.val.u64_or("blobs", 0),
            hits.len(),
            pw.xml_attrs(),
            self.listing().xml_escaped(),
            self.val.str_or("ref_labels", "parsed"),
            self.val
                .opt_str("at")
                .filter(|s| !s.is_empty())
                .unwrap_or("HEAD"),
            self.val.str_or("worktree", "read"),
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        use crate::serialize::PaginateSlice;
        let sym = escape_xml_str(self.val.str_or("symbol", ""));
        if let Some(hist) = self.val.get("history").and_then(Value::as_object) {
            emit_whereis_history(out, &Value::Object(hist.clone()), &sym)?;
        }
        let rows = whereis_rows(self.hits(), self.listing());
        let (_, shown) = rows.listed.paginate(60, None, None);
        for &(h, refs) in shown {
            emit_whereis_hit(out, h, refs)?;
        }
        if self.listing() == "defs" && rows.ref_rows > 0 {
            writeln!(
                out,
                "  <refs count=\"{}\" next=\"--whereis={sym} --whereis-listing=refs\"/>",
                rows.ref_rows
            )?;
        }
        Ok(())
    }
}

pub fn serialize_whereis_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    WhereisReport { val }.render_envelope(out)
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

fn collect_owner_display_items(files: &[Value], detail: bool) -> Vec<&Value> {
    files
        .iter()
        .filter(|f| detail || f.u64_or("authors", 1) > 1)
        .collect()
}

/// Serialize `owners` authorship to Ripwire XML.
pub struct OwnersReport<'a> {
    pub val: &'a Value,
}

impl crate::serialize::XmlReport for OwnersReport<'_> {
    crate::xml_report_meta!("owners", "ripwire-rs.owners/v1", root: false);
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let files = self.val.array_any(&["files", "ownership"]);
        let count = files.map_or(0, Vec::len);
        let root = escape_xml_str(self.val.str_or("root", "."));
        let at_attr = match self.val.opt_str("at") {
            Some(at) if !at.is_empty() => format!(" at=\"{at}\""),
            _ => String::new(),
        };
        let detail = self.val.bool_or("detail", false);
        let display_items = files
            .map(|f| collect_owner_display_items(f, detail))
            .unwrap_or_default();
        let (pw, _) = self.val.paginate_items(&display_items, 40);
        let page_attrs = pw.xml_attrs();
        write!(
            out,
            " files=\"{count}\"{page_attrs} root=\"{root}\"{at_attr}"
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let files = self.val.array_any(&["files", "ownership"]);
        let detail = self.val.bool_or("detail", false);
        let uniform_count = files.map_or(0, |f| count_uniform_owners(f));
        let display_items = files
            .map(|f| collect_owner_display_items(f, detail))
            .unwrap_or_default();
        let (_, page) = self.val.paginate_items(&display_items, 40);
        if !detail && uniform_count > 0 {
            writeln!(
                out,
                "  <uniform authors=\"1\" bf=\"1\" share=\"1.00\" files=\"{uniform_count}\"/>"
            )?;
        }
        for f in page {
            write_owner_entry(out, f)?;
        }
        Ok(())
    }
}

pub fn serialize_owners_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    OwnersReport { val }.render_envelope(out)
}

/// Serialize `flags` environment and compile gates to Ripwire XML.
fn write_flag_gate_entry<W: Write>(out: &mut W, g: &Value) -> io::Result<()> {
    let name = g.str_any(&["name", "flag"], "").xml_escaped();
    let kind = g.str_or("kind", "compile");
    let file = g.str_any(&["file", "path"], "").xml_escaped();
    let line = g.u64_or("line", 0);
    let def_attr = match g.opt_str("default").filter(|s| !s.is_empty()) {
        Some(d) => format!(" default=\"{}\"", d.xml_escaped()),
        None => String::new(),
    };
    let dark_attr = g
        .get("dark")
        .and_then(Value::as_u64)
        .map_or_else(String::new, |d| format!(" dark=\"{d}\""));
    let reg_attr = g
        .get("regions")
        .and_then(Value::as_u64)
        .map_or_else(String::new, |r| format!(" regions=\"{r}\""));
    let loc_attr = g
        .get("loc")
        .and_then(Value::as_u64)
        .map_or_else(String::new, |l| format!(" loc=\"{l}\""));
    let reads = g["reads"].as_array();
    let reads_attr = g
        .get("reads_count")
        .and_then(Value::as_u64)
        .or_else(|| reads.map(|r| r.len() as u64))
        .map_or_else(String::new, |c| format!(" reads=\"{c}\""));
    if let Some(read_list) = reads.filter(|r| !r.is_empty()) {
        writeln!(
            out,
            "  <gate name=\"{name}\" kind=\"{kind}\"{def_attr}{dark_attr}{reg_attr}{loc_attr}{reads_attr} p=\"{file}\" l=\"{line}\">"
        )?;
        for r in read_list {
            let rp = r.str_any(&["path", "file"], "").xml_escaped();
            let rl = r.u64_or("line", 1);
            writeln!(out, "    <read p=\"{rp}\" l=\"{rl}\"/>")?;
        }
        writeln!(out, "  </gate>")
    } else {
        writeln!(
            out,
            "  <gate name=\"{name}\" kind=\"{kind}\"{def_attr}{dark_attr}{reg_attr}{loc_attr}{reads_attr} p=\"{file}\" l=\"{line}\"/>"
        )
    }
}

pub struct FlagsReport<'a> {
    pub val: &'a Value,
}

impl crate::serialize::XmlReport for FlagsReport<'_> {
    crate::xml_report_meta!("flags", "ripwire-rs.flags/v1", root: false);
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let empty = Vec::new();
        let items = self.val.array_any(&["gates", "flags"]).unwrap_or(&empty);
        let count = items.len();
        let files = self.val.u64_or("files", 0);
        let compile = self.val.u64_or("compile_count", 0);
        let cmake = self.val.u64_or("cmake_count", 0);
        let env = self.val.u64_or("env_count", 0);
        let dark = self.val.u64_or("dark_gates", 0);
        let (pw, _) = self.val.paginate_items(items, 100);
        let page_attrs = pw.xml_attrs();
        write!(
            out,
            " files=\"{files}\" gates=\"{count}\" dark_gates=\"{dark}\" compile=\"{compile}\" cmake=\"{cmake}\" env=\"{env}\"{page_attrs}"
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let empty = Vec::new();
        let items = self.val.array_any(&["gates", "flags"]).unwrap_or(&empty);
        let (_, page) = self.val.paginate_items(items, 100);
        for g in page {
            write_flag_gate_entry(out, g)?;
        }
        Ok(())
    }
}

pub fn serialize_flags_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    FlagsReport { val }.render_envelope(out)
}

/// Serialize `doc_drift` out-of-date document references to Ripwire XML.
fn format_doc_drift_extra_attrs(d: &Value) -> String {
    let mut extra = String::new();
    for key in ["sym", "got", "tgt", "fate", "commit", "date", "subject"] {
        if let Some(v) = d[key].as_str()
            && !v.is_empty()
        {
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

fn format_doc_drift_header_counts(val: &Value) -> String {
    let Some(clean) = val["clean_docs"].as_u64() else {
        return String::new();
    };
    let dated = val.u64_or("dated_docs", 0);
    let prose_attr = val["prose_docs"]
        .as_u64()
        .map_or_else(String::new, |p| format!(" prose=\"{p}\""));
    let corpus_attr = val["corpus"]
        .as_u64()
        .map_or_else(String::new, |c| format!(" corpus=\"{c}\""));
    let anchor_attrs = match val["anchors"].as_u64() {
        Some(anchors) => {
            let checked = val.u64_or("checked", 0);
            let unchecked = val.u64_or("unchecked", anchors.saturating_sub(checked));
            format!(" anchors=\"{anchors}\" checked=\"{checked}\" unchecked=\"{unchecked}\"")
        }
        None => String::new(),
    };
    format!(" clean=\"{clean}\" dated=\"{dated}\"{prose_attr}{corpus_attr}{anchor_attrs}")
}

fn group_doc_drift_items(val: &Value) -> Vec<(&str, Vec<&Value>)> {
    let mut grouped: Vec<(&str, Vec<&Value>)> = Vec::new();
    if let Some(items) = val["drift"].as_array() {
        for d in items {
            let doc = d["doc"].as_str().unwrap_or("");
            if let Some((_, list)) = grouped.iter_mut().find(|(p, _)| *p == doc) {
                list.push(d);
            } else {
                grouped.push((doc, vec![d]));
            }
        }
    }
    grouped
}

pub struct DocDriftReport<'a> {
    pub val: &'a Value,
}

impl crate::serialize::XmlReport for DocDriftReport<'_> {
    crate::xml_report_meta!("doc-drift", "ripwire-rs.doc-drift/v1", root: false);
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        use crate::serialize::GitStampExt;
        let count = self.val["drift"].as_array().map_or(0, Vec::len);
        let raw_root = self.val["root"].as_str().unwrap_or(".");
        let root = escape_xml_str(raw_root);
        let at_attr = raw_root.git_at_xml_attr();
        let grouped = group_doc_drift_items(self.val);
        let docs_cnt = self.val["docs_scanned"]
            .as_u64()
            .map_or(grouped.len(), |n| n as usize);
        let extra_counts = format_doc_drift_header_counts(self.val);
        let (pw, _) = self.val.paginate_items(&grouped, 40);
        let page_attrs = pw.xml_attrs();
        write!(
            out,
            " root=\"{root}\" docs=\"{docs_cnt}\"{extra_counts} drift=\"{count}\"{page_attrs}{at_attr}"
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let grouped = group_doc_drift_items(self.val);
        let (_, page) = self.val.paginate_items(&grouped, 40);
        for (doc, list) in page {
            let file = escape_xml_str(doc);
            writeln!(out, "  <doc p=\"{file}\" drift=\"{}\">", list.len())?;
            for d in list {
                write_doc_drift_anchor(out, &file, d)?;
            }
            writeln!(out, "  </doc>")?;
        }
        Ok(())
    }
}

pub fn serialize_doc_drift_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    DocDriftReport { val }.render_envelope(out)
}

/// Serialize `stray_content` branch divergence to Ripwire XML.
pub struct StrayContentReport<'a> {
    pub val: &'a Value,
}

impl crate::serialize::XmlReport for StrayContentReport<'_> {
    crate::xml_report_meta!("stray-content", "ripwire-rs.stray-content/v1", root: false);
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let head = self.val.str_or("head", "").xml_escaped();
        let head_ref = self.val.str_or("head_ref", "HEAD").xml_escaped();
        let refs = self.val.u64_or("refs", 0);
        let blobs = self.val.u64_or("blobs", 0);
        let unmerged = self.val.u64_or("unmerged", 0);
        let superseded = self.val.u64_or("superseded", 0);
        let merged = self.val.u64_or("merged", 0);
        let unknown = self.val.u64_or("unknown", 0);
        write!(
            out,
            " head=\"{head}\" head_ref=\"{head_ref}\" refs=\"{refs}\" blobs=\"{blobs}\" unmerged=\"{unmerged}\" superseded=\"{superseded}\" merged=\"{merged}\" unknown=\"{unknown}\""
        )?;
        if let Some(at) = self.val.opt_str("at").filter(|s| !s.is_empty()) {
            write!(out, " at=\"{}\"", at.xml_escaped())?;
        }
        Ok(())
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        if let Some(branches) = self.val["branches"].as_array() {
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
    }
}

pub fn serialize_stray_content_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    StrayContentReport { val }.render_envelope(out)
}

/// Serialize `mentions` doc backtick occurrences to Ripwire XML.
pub struct MentionsReport<'a> {
    pub val: &'a Value,
}

impl crate::serialize::XmlReport for MentionsReport<'_> {
    crate::xml_report_meta!("mentions", "ripwire-rs.mentions/v1", root: false);
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let sym = self.val.str_or("symbol", "").xml_escaped();
        let count = self.val["docs"].as_array().map_or(0, Vec::len);
        write!(out, " of=\"{sym}\" root=\".\" docs=\"{count}\"")
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        if let Some(items) = self.val["docs"].as_array() {
            for d in items {
                let path = d.str_any(&["path", "file"], "").xml_escaped();
                let mentions = d.u64_or("mentions", 0);
                writeln!(out, "  <doc p=\"{path}\" mentions=\"{mentions}\"/>")?;
            }
        }
        Ok(())
    }
}

pub fn serialize_mentions_xml<W: Write>(val: &Value, out: &mut W) -> io::Result<()> {
    use crate::serialize::XmlReport;
    MentionsReport { val }.render_envelope(out)
}

/// Serialize `--expand` symbol bodies to Ripwire XML.
pub fn serialize_expand_xml<W: Write>(
    root: &str,
    items: &[(&crate::domain::Symbol, &str, &str, &str)],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<ctx schema=\"ripwire-rs.expand/v1\" root=\"{root}\"><bodies shown=\"{}\" total=\"{}\">",
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
pub struct TreeReport<'a> {
    pub root: &'a str,
    pub file_symbols: &'a [TreeFileSymbols<'a>],
    pub files_unlisted: usize,
    pub pr_iters: u32,
    pub page: crate::serialize::PageRequest,
}

impl crate::serialize::XmlReport for TreeReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "tree"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire-rs.tree/v1"
    }
    fn root_path(&self) -> &str {
        self.root
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire-rs tree schema=ripwire-rs.tree/v1: each file with its top 3 symbols by rank, files by best symbol: <file p= symbols=> of <s t= n=>.",
        )
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let total_files = self.file_symbols.len() + self.files_unlisted;
        let (pw, shown_slice) = self.shown();
        let total_shown: usize = shown_slice.iter().map(|f| f.top_symbols.len()).sum();
        let symbols_capped = usize::from(
            shown_slice
                .iter()
                .any(|f| f.total_symbols > f.top_symbols.len()),
        );
        let page_attrs = pw.xml_attrs();
        let next_attr = if pw.has_more {
            format!(" next=\"--tree --offset={}\"", pw.next_offset)
        } else {
            String::new()
        };
        write!(
            out,
            " files=\"{total_files}\" files_unlisted=\"{}\"{page_attrs} pr_iters=\"{}\" root=\"{}\" shown_symbols=\"{total_shown}\" symbols_capped=\"{symbols_capped}\"{next_attr}",
            self.files_unlisted,
            self.pr_iters,
            self.root.xml_escaped()
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let (_, shown_slice) = self.shown();
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
        Ok(())
    }
}

impl TreeReport<'_> {
    fn shown(&self) -> (crate::serialize::PageWindow, &[TreeFileSymbols<'_>]) {
        use crate::serialize::PaginateSlice;
        self.file_symbols
            .paginate(80, self.page.limit, self.page.offset)
    }
}

/// Serialize `--pack-top-n` bodies to Ripwire XML.
pub fn serialize_pack_top_n_xml<W: Write>(
    root: &str,
    bodies: &[(&str, &str)],
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<ctx schema=\"ripwire-rs.pack-top-n/v1\" root=\"{root}\">"
    )?;
    for (path, body) in bodies {
        writeln!(out, "<src p=\"{}\">\n{body}\n</src>", path.xml_escaped())?;
    }
    writeln!(out, "</ctx>")
}
