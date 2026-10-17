pub mod escape;
pub mod html;
pub mod query_xml;
pub mod writer;
pub mod xml;

pub use writer::{MinifyingWriter, XmlWriter};

pub use query_xml::{
    CommunityModule, CommunitySummary, DepsFileRow, DepsReport, LEGEND_DICT_ENTRIES,
    LEGEND_DICT_VERSION, TreeFileSymbols, serialize_around_xml,
    serialize_communities_with_summary_xml, serialize_communities_xml, serialize_connect_xml,
    serialize_dead_code_xml, serialize_deps_report_xml, serialize_deps_xml,
    serialize_doc_drift_xml, serialize_exemplar_xml, serialize_exercises_xml, serialize_expand_xml,
    serialize_external_surface_xml, serialize_flags_xml, serialize_for_xml,
    serialize_from_trace_xml, serialize_grep_xml, serialize_legend_dict_xml, serialize_lego_xml,
    serialize_mentions_xml, serialize_owners_xml, serialize_pack_top_n_xml,
    serialize_path_between_xml, serialize_recall_xml, serialize_safe_delete_xml,
    serialize_stray_content_xml, serialize_test_gate_xml, serialize_tree_xml,
    serialize_whereis_xml,
};

pub use html::{ColorBy, generate_call_graph_html, write_call_graph_html};

pub use escape::{
    XmlEscapeExt, XmlEscaped, escape_xml_str, escape_xml_write, needs_escaping, xml_safe_byte,
};
pub use xml::{
    CountingWriter, DEFAULT_PACK_BUDGET_BYTES, DEFAULT_STREAM_BUFFER_CAPACITY, RootElementContext,
    SerializeOptions, StreamXmlContext, bucket_symbols_by_rank, serialize_xml,
    serialize_xml_to_string, serialize_xml_with_ranks, write_file_header, write_root_open,
    write_root_open_with_rank_by, write_symbol_row,
};

/// Computed pagination slice and Ripwire truncation disclosure metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageWindow {
    pub begin: usize,
    pub end: usize,
    pub shown: usize,
    pub total: usize,
    pub capped: bool,
    pub has_more: bool,
    pub next_offset: usize,
    pub limit: usize,
    pub offset: usize,
}

impl PageWindow {
    /// Compute a pagination window over `total` items given a `default_cap` and optional `limit` / `offset`.
    #[must_use]
    pub fn compute(
        total: usize,
        default_cap: usize,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Self {
        let off = offset.unwrap_or(0);
        let lim = limit.unwrap_or(0);
        let eff_cap = if lim > 0 { lim } else { default_cap };
        let begin = off.min(total);
        let end = if eff_cap > 0 {
            (begin + eff_cap).min(total)
        } else {
            total
        };
        let shown = end.saturating_sub(begin);
        let has_more = end < total;
        let capped = shown < total;
        Self {
            begin,
            end,
            shown,
            total,
            capped,
            has_more,
            next_offset: end,
            limit: lim,
            offset: off,
        }
    }

    /// Format Ripwire XML pagination disclosure attributes (`shown=`, `capped=`, `total=`, `has_more=`, `next_offset=`).
    #[must_use]
    pub fn xml_attrs(&self) -> String {
        if self.capped || self.offset > 0 || self.limit > 0 {
            let hm = usize::from(self.has_more);
            let cap = usize::from(self.capped);
            format!(
                " shown=\"{}\" capped=\"{cap}\" total=\"{}\" has_more=\"{hm}\" next_offset=\"{}\" offset=\"{}\" limit=\"{}\"",
                self.shown, self.total, self.next_offset, self.offset, self.limit
            )
        } else {
            format!(" shown=\"{}\" capped=\"0\"", self.shown)
        }
    }
}

/// Trait for types that can be serialized as XML and converted to a string.
pub trait ToXmlString {
    fn write_xml<W: std::io::Write>(&self, out: &mut W) -> std::io::Result<()>;

    fn to_xml_string(&self) -> String {
        let mut buf = Vec::new();
        let _ = self.write_xml(&mut buf);
        String::from_utf8(buf).unwrap_or_default()
    }
}

impl<T: XmlReport + ?Sized> ToXmlString for T {
    fn write_xml<W: std::io::Write>(&self, out: &mut W) -> std::io::Result<()> {
        self.render_envelope(out)
    }
}

/// Resolver gauge counts (`graph_ambiguous=`, `graph_unresolved=`, `graph_unindexed=`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GraphGauges {
    pub ambiguous: usize,
    pub unresolved: usize,
    pub unindexed: usize,
}

/// Unified XML envelope trait for structured Ripwire reports.
pub trait XmlReport {
    fn schema_tag(&self) -> &'static str;
    fn schema_version(&self) -> &'static str;
    fn root_path(&self) -> &str {
        "."
    }
    fn include_root_attr(&self) -> bool {
        true
    }
    fn legend_comment(&self) -> Option<&'static str> {
        None
    }
    fn page_window(&self) -> Option<PageWindow> {
        None
    }
    fn pr_iters(&self) -> Option<u32> {
        None
    }
    fn graph_gauges(&self) -> Option<GraphGauges> {
        None
    }
    fn write_extra_attrs<W: std::io::Write>(&self, _out: &mut W) -> std::io::Result<()> {
        Ok(())
    }
    fn write_body<W: std::io::Write>(&self, out: &mut W) -> std::io::Result<()>;

    fn write_xml_report<W: std::io::Write>(&self, root: &str, out: &mut W) -> std::io::Result<()> {
        let tag = self.schema_tag();
        let ver = self.schema_version();
        if let Some(comment) = self.legend_comment() {
            writeln!(out, "<!-- {comment} -->")?;
        }
        if self.include_root_attr() {
            write!(
                out,
                "<{tag} schema=\"{ver}\" root=\"{}\"",
                root.xml_escaped()
            )?;
        } else {
            write!(out, "<{tag} schema=\"{ver}\"")?;
        }
        if let Some(pw) = self.page_window() {
            out.write_all(pw.xml_attrs().as_bytes())?;
        }
        if let Some(iters) = self.pr_iters() {
            write!(out, " pr_iters=\"{}\"", iters.max(1))?;
        }
        self.write_extra_attrs(out)?;
        if let Some(g) = self.graph_gauges() {
            write!(
                out,
                " graph_ambiguous=\"{}\" graph_unresolved=\"{}\" graph_unindexed=\"{}\" counts_floor=\"1\"",
                g.ambiguous, g.unresolved, g.unindexed
            )?;
        }
        writeln!(out, ">")?;
        self.write_body(out)?;
        writeln!(out, "</{tag}>")
    }

    fn render_envelope<W: std::io::Write>(&self, out: &mut W) -> std::io::Result<()> {
        self.write_xml_report(self.root_path(), out)
    }
}

/// Extension trait for concise field extraction from `serde_json::Value`.
pub trait JsonValueExt {
    fn req_str(&self, key: &str) -> Result<&str, String>;
    fn req_str_any(&self, keys: &[&str]) -> Result<&str, String>;
    fn opt_str(&self, key: &str) -> Option<&str>;
    fn opt_str_any(&self, keys: &[&str]) -> Option<&str>;
    fn str_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str;
    fn str_any<'a>(&'a self, keys: &[&str], default: &'a str) -> &'a str;
    fn u64_or(&self, key: &str, default: u64) -> u64;
    fn u64_any(&self, keys: &[&str], default: u64) -> u64;
    fn opt_usize(&self, key: &str) -> Option<usize>;
    fn bool_or(&self, key: &str, default: bool) -> bool;
    fn f64_any(&self, keys: &[&str], default: f64) -> f64;
    fn array_any(&self, keys: &[&str]) -> Option<&Vec<serde_json::Value>>;
    fn str_vec(&self, key: &str) -> Vec<String>;
}

impl JsonValueExt for serde_json::Value {
    #[inline]
    fn req_str(&self, key: &str) -> Result<&str, String> {
        self.get(key)
            .and_then(Self::as_str)
            .ok_or_else(|| format!("missing '{key}' parameter"))
    }

    #[inline]
    fn req_str_any(&self, keys: &[&str]) -> Result<&str, String> {
        self.opt_str_any(keys).ok_or_else(|| {
            let names = keys
                .iter()
                .map(|k| format!("'{k}'"))
                .collect::<Vec<_>>()
                .join(" or ");
            format!("missing {names} parameter")
        })
    }

    #[inline]
    fn opt_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Self::as_str)
    }

    #[inline]
    fn opt_str_any(&self, keys: &[&str]) -> Option<&str> {
        keys.iter()
            .find_map(|&k| self.get(k).and_then(Self::as_str))
    }

    #[inline]
    fn str_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.opt_str(key).unwrap_or(default)
    }

    #[inline]
    fn str_any<'a>(&'a self, keys: &[&str], default: &'a str) -> &'a str {
        self.opt_str_any(keys).unwrap_or(default)
    }

    #[inline]
    fn u64_or(&self, key: &str, default: u64) -> u64 {
        self.get(key).and_then(Self::as_u64).unwrap_or(default)
    }

    #[inline]
    fn u64_any(&self, keys: &[&str], default: u64) -> u64 {
        keys.iter()
            .find_map(|&k| self.get(k).and_then(Self::as_u64))
            .unwrap_or(default)
    }

    #[inline]
    fn opt_usize(&self, key: &str) -> Option<usize> {
        self.get(key).and_then(Self::as_u64).map(|v| v as usize)
    }

    #[inline]
    fn bool_or(&self, key: &str, default: bool) -> bool {
        self.get(key).and_then(Self::as_bool).unwrap_or(default)
    }

    #[inline]
    fn f64_any(&self, keys: &[&str], default: f64) -> f64 {
        keys.iter()
            .find_map(|&k| self.get(k).and_then(Self::as_f64))
            .unwrap_or(default)
    }

    #[inline]
    fn array_any(&self, keys: &[&str]) -> Option<&Vec<serde_json::Value>> {
        keys.iter()
            .find_map(|&k| self.get(k).and_then(Self::as_array))
    }

    #[inline]
    fn str_vec(&self, key: &str) -> Vec<String> {
        self.get(key)
            .and_then(Self::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }
}
