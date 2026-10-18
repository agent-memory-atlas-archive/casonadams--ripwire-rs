use super::{ImpactNode, NavNeighborhood, SymbolNavInfo};
use crate::domain::{IngestResult, Interner};
use crate::serialize::XmlEscapeExt;
use std::collections::BTreeSet;
use std::io::{self, Write};

fn format_nav_page_attrs(_shown: usize, total: usize) -> String {
    crate::serialize::PageWindow::compute(total, 40, None, None).xml_attrs()
}

pub struct NavItemsReport<'a> {
    pub tag: &'static str,
    pub root: &'a str,
    pub of: &'a str,
    pub items: &'a [SymbolNavInfo],
    pub next_flag: &'static str,
    pub gauge_attrs: &'a str,
}

impl crate::serialize::XmlReport for NavItemsReport<'_> {
    fn schema_tag(&self) -> &'static str {
        self.tag
    }
    fn schema_version(&self) -> &'static str {
        if self.tag == "callers" {
            "ripwire.callers/v1"
        } else {
            "ripwire.callees/v1"
        }
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        if self.tag == "callers" {
            Some("ripwire callers schema=ripwire.callers/v1: 1-hop callers of of=.")
        } else {
            Some("ripwire callees schema=ripwire.callees/v1: 1-hop callees of of=.")
        }
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let count = self.items.len();
        let shown = count.min(40);
        let page_attrs = format_nav_page_attrs(shown, count);
        let hop_tested = self
            .items
            .iter()
            .filter(|it| it.tested || crate::gate::affected::is_test_path(&it.path))
            .count();
        let hop_untested = count.saturating_sub(hop_tested);
        write!(
            out,
            " of=\"{}\" defs=\"1\" count=\"{count}\" root=\"{}\" hop_tested=\"{hop_tested}\" hop_untested=\"{hop_untested}\"{page_attrs} {} next=\"{}={}\"",
            self.of, self.root, self.gauge_attrs, self.next_flag, self.of
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let shown = self.items.len().min(40);
        let via_attr = if self.tag == "callees" {
            " via=\"name\""
        } else {
            ""
        };
        for it in self.items.iter().take(shown) {
            let tested_attr = if it.tested || crate::gate::affected::is_test_path(&it.path) {
                " tested=\"1\""
            } else {
                ""
            };
            let sites_attr = if self.tag == "callers" && !it.sites.is_empty() {
                format!(" sites_at=\"{}\"", it.sites.join(" ").xml_escaped())
            } else {
                String::new()
            };
            write!(
                out,
                "<s t=\"{}\" n=\"{}\" p=\"{}:{}\"{tested_attr}{sites_attr}{via_attr}/>",
                it.kind, it.name, it.path, it.line
            )?;
        }
        Ok(())
    }
}

fn write_nav_items<W: Write>(
    tag: &'static str,
    root: &str,
    of: &str,
    items: &[SymbolNavInfo],
    next_flag: &'static str,
    gauge_attrs: &str,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    NavItemsReport {
        tag,
        root,
        of,
        items,
        next_flag,
        gauge_attrs,
    }
    .render_envelope(out)
}

/// Serialize direct callers to Ripwire-compatible XML.
pub fn serialize_callers_xml<W: Write>(
    sym_name: &str,
    callers: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    serialize_callers_in_root_xml(".", sym_name, callers, out)
}

pub fn serialize_callers_in_root_xml<W: Write>(
    root: &str,
    sym_name: &str,
    callers: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    write_nav_items(
        "callers",
        root,
        sym_name,
        callers,
        "--uses",
        crate::serialize::GraphGauges::ZERO_XML_ATTRS,
        out,
    )
}

pub fn serialize_callers_with_gauge_xml<W: Write>(
    root: &str,
    sym_name: &str,
    callers: &[SymbolNavInfo],
    gauge_attrs: &str,
    out: &mut W,
) -> io::Result<()> {
    write_nav_items(
        "callers",
        root,
        sym_name,
        callers,
        "--uses",
        gauge_attrs,
        out,
    )
}

/// Serialize direct callees to Ripwire-compatible XML.
pub fn serialize_callees_xml<W: Write>(
    sym_name: &str,
    callees: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    serialize_callees_in_root_xml(".", sym_name, callees, out)
}

pub fn serialize_callees_in_root_xml<W: Write>(
    root: &str,
    sym_name: &str,
    callees: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    write_nav_items(
        "callees",
        root,
        sym_name,
        callees,
        "--expand",
        crate::serialize::GraphGauges::ZERO_XML_ATTRS,
        out,
    )
}

pub fn serialize_callees_with_gauge_xml<W: Write>(
    root: &str,
    sym_name: &str,
    callees: &[SymbolNavInfo],
    gauge_attrs: &str,
    out: &mut W,
) -> io::Result<()> {
    write_nav_items(
        "callees",
        root,
        sym_name,
        callees,
        "--expand",
        gauge_attrs,
        out,
    )
}

/// Serialize transitive impact blast radius to Ripwire-compatible XML.
pub fn serialize_impact_xml<W: Write>(
    sym_name: &str,
    nodes: &[ImpactNode],
    out: &mut W,
) -> io::Result<()> {
    serialize_impact_in_root_xml(".", sym_name, nodes, out)
}

pub fn serialize_impact_in_root_xml<W: Write>(
    root: &str,
    sym_name: &str,
    nodes: &[ImpactNode],
    out: &mut W,
) -> io::Result<()> {
    serialize_impact_with_gauge_xml(
        root,
        sym_name,
        nodes,
        crate::serialize::GraphGauges::ZERO_XML_ATTRS,
        1,
        out,
    )
}

pub struct ImpactReport<'a> {
    pub root: &'a str,
    pub sym_name: &'a str,
    pub nodes: &'a [ImpactNode],
    pub importers: Vec<&'a str>,
    pub gauge_attrs: &'a str,
    pub pr_iters: u32,
}

impl<'a> ImpactReport<'a> {
    fn effective_importers(&self) -> BTreeSet<&'a str> {
        let mut set: BTreeSet<&'a str> = self.importers.iter().copied().collect();
        for n in self.nodes {
            if n.hops <= 1 && !n.path.is_empty() {
                set.insert(&n.path);
            }
        }
        set
    }
}

impl crate::serialize::XmlReport for ImpactReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "impact"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.impact/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some("ripwire impact schema=ripwire.impact/v1: transitive blast radius of of=.")
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let reaches = self.nodes.len();
        let shown = reaches.min(40);
        let capped = usize::from(shown < reaches);
        let radius_tested = self
            .nodes
            .iter()
            .filter(|n| n.tested || crate::gate::affected::is_test_path(&n.path))
            .count();
        let radius_untested = reaches.saturating_sub(radius_tested);
        let mut depth_counts: std::collections::BTreeMap<u32, usize> =
            std::collections::BTreeMap::new();
        for n in self.nodes {
            *depth_counts.entry(n.hops).or_insert(0) += 1;
        }
        let by_depth = depth_counts
            .iter()
            .map(|(d, c)| format!("{d}:{c}"))
            .collect::<Vec<_>>()
            .join(",");
        let importer_paths = self.effective_importers();
        let imp_total = importer_paths.len();
        let imp_shown = imp_total.min(37);
        let imp_capped = usize::from(imp_shown < imp_total);
        write!(
            out,
            " of=\"{}\" defs=\"1\" reaches=\"{reaches}\" by_depth=\"{by_depth}\" importers=\"{imp_total}\" shown_importers=\"{imp_shown}\" importers_capped=\"{imp_capped}\" radius_tested=\"{radius_tested}\" radius_untested=\"{radius_untested}\" root=\"{}\" shown=\"{shown}\" capped=\"{capped}\" {} pr_iters=\"{}\" next=\"--safe-delete={}\"",
            self.sym_name,
            self.root,
            self.gauge_attrs,
            self.pr_iters.max(1),
            self.sym_name
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let shown = self.nodes.len().min(40);
        for n in self.nodes.iter().take(shown) {
            let tested_attr = if n.tested || crate::gate::affected::is_test_path(&n.path) {
                " tested=\"1\""
            } else {
                ""
            };
            write!(
                out,
                "<s t=\"{}\" n=\"{}\" p=\"{}:{}\" d=\"{}\" dist=\"{}\"{tested_attr}/>",
                n.kind, n.name, n.path, n.line, n.hops, n.hops
            )?;
        }
        for p in self.effective_importers().into_iter().take(37) {
            write!(
                out,
                "<f via=\"import\" p=\"{}\" lazy=\"0\"/>",
                p.xml_escaped()
            )?;
        }
        Ok(())
    }
}

pub fn serialize_impact_with_gauge_xml<W: Write>(
    root: &str,
    sym_name: &str,
    nodes: &[ImpactNode],
    gauge_attrs: &str,
    pr_iters: u32,
    out: &mut W,
) -> io::Result<()> {
    serialize_impact_with_importers_xml(
        root,
        sym_name,
        nodes,
        Vec::new(),
        gauge_attrs,
        pr_iters,
        out,
    )
}

pub fn serialize_impact_with_importers_xml<'a, W: Write>(
    root: &'a str,
    sym_name: &'a str,
    nodes: &'a [ImpactNode],
    importers: Vec<&'a str>,
    gauge_attrs: &'a str,
    pr_iters: u32,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    ImpactReport {
        root,
        sym_name,
        nodes,
        importers,
        gauge_attrs,
        pr_iters,
    }
    .render_envelope(out)
}

/// Serialize symbol neighborhood to Ripwire-compatible XML.
pub fn serialize_find_symbol_xml<W: Write>(nav: &NavNeighborhood, out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "<find-symbol schema=\"ripwire.find-symbol/v1\" of=\"{}\" p=\"{}:{}\" callers=\"{}\" callees=\"{}\">",
        nav.target.name,
        nav.target.path,
        nav.target.line,
        nav.callers.len(),
        nav.callees.len()
    )?;
    writeln!(out, "  <callers>")?;
    for c in &nav.callers {
        writeln!(
            out,
            "    <s t=\"{}\" n=\"{}\" p=\"{}:{}\"/>",
            c.kind, c.name, c.path, c.line
        )?;
    }
    writeln!(out, "  </callers>")?;
    writeln!(out, "  <callees>")?;
    for c in &nav.callees {
        writeln!(
            out,
            "    <s t=\"{}\" n=\"{}\" p=\"{}:{}\"/>",
            c.kind, c.name, c.path, c.line
        )?;
    }
    writeln!(out, "  </callees>")?;
    writeln!(out, "</find-symbol>")
}

/// Serialize static use-sites to Ripwire-compatible XML.
pub fn serialize_uses_xml<W: Write>(
    sym_name: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    serialize_uses_in_root_xml(".", sym_name, ingest, interner, out)
}

pub fn serialize_uses_in_root_xml<W: Write>(
    root: &str,
    sym_name: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    serialize_uses_with_gauge_xml(
        root,
        sym_name,
        ingest,
        interner,
        crate::serialize::GraphGauges::ZERO_XML_ATTRS,
        out,
    )
}

pub struct UsesReport<'a> {
    pub root: &'a str,
    pub sym_name: &'a str,
    pub sites: Vec<(&'static str, &'a str, u32, &'a str)>,
    pub gauge_attrs: &'a str,
}

impl crate::serialize::XmlReport for UsesReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "uses"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.uses/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some("ripwire uses schema=ripwire.uses/v1: resolvable use-sites of of=.")
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let total = self.sites.len();
        let shown = total.min(40);
        let page_attrs = format_nav_page_attrs(shown, total);
        write!(
            out,
            " of=\"{}\" defs=\"1\" external=\"0\" count=\"{total}\" root=\"{}\"{page_attrs} {}",
            self.sym_name, self.root, self.gauge_attrs
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for &(role, file, line, enc) in self.sites.iter().take(40) {
            if enc.is_empty() {
                writeln!(out, "  <u role=\"{}\" p=\"{}:{}\"/>", role, file, line)?;
            } else {
                writeln!(
                    out,
                    "  <u role=\"{}\" p=\"{}:{}\" in_id=\"{}\"/>",
                    role, file, line, enc
                )?;
            }
        }
        Ok(())
    }
}

pub fn serialize_uses_with_gauge_xml<W: Write>(
    root: &str,
    sym_name: &str,
    ingest: &IngestResult,
    interner: &Interner,
    gauge_attrs: &str,
    out: &mut W,
) -> io::Result<()> {
    use crate::domain::CorpusView;
    use crate::serialize::XmlReport;
    let view = (ingest, interner);
    let mut sites = Vec::new();
    for r in &ingest.references {
        if view.reference_matches_query(r, sym_name) {
            let file_path = view.file_path(r.file_id);
            let enclosing = if r.from_symbol != crate::domain::NO_NODE {
                ingest.sym(r.from_symbol).map_or("", |s| view.sym_name(s))
            } else {
                ""
            };
            sites.push((r.role.tag(), file_path, r.line, enclosing));
        }
    }
    UsesReport {
        root,
        sym_name,
        sites,
        gauge_attrs,
    }
    .render_envelope(out)
}
