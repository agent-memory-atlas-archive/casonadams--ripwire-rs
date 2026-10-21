use super::{ImpactNode, NavNeighborhood, SymbolNavInfo};
use crate::domain::{IngestResult, Interner};
use crate::serialize::XmlEscapeExt;
use std::collections::BTreeSet;
use std::io::{self, Write};

pub struct NavItemsReport<'a> {
    pub tag: &'static str,
    pub root: &'a str,
    pub of: &'a str,
    pub defs: usize,
    pub items: &'a [SymbolNavInfo],
    pub gauge_attrs: &'a str,
    pub page: crate::serialize::PageRequest,
}

impl NavItemsReport<'_> {
    fn window(&self) -> crate::serialize::PageWindow {
        self.page.window(self.items.len(), 40)
    }
}

impl crate::serialize::XmlReport for NavItemsReport<'_> {
    fn schema_tag(&self) -> &'static str {
        self.tag
    }
    fn schema_version(&self) -> &'static str {
        if self.tag == "callers" {
            "ripwire-rs.callers/v1"
        } else {
            "ripwire-rs.callees/v1"
        }
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        if self.tag == "callers" {
            Some("ripwire-rs callers schema=ripwire-rs.callers/v1: 1-hop callers of of=.")
        } else {
            Some("ripwire-rs callees schema=ripwire-rs.callees/v1: 1-hop callees of of=.")
        }
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let count = self.items.len();
        let win = self.window();
        let hop_tested = self
            .items
            .iter()
            .filter(|it| it.tested || crate::gate::affected::is_test_path(&it.path))
            .count();
        let hop_untested = count.saturating_sub(hop_tested);
        let of = self.of.xml_escaped();
        let next = match (win.has_more, self.tag) {
            (true, tag) => format!(
                " next=\"--{tag}={of} --offset={}\"{}",
                win.next_offset,
                if tag == "callers" {
                    format!(" uses_next=\"--uses={of}\"")
                } else {
                    String::new()
                }
            ),
            (false, "callers") => format!(" next=\"--uses={of}\""),
            (false, _) => format!(" next=\"--expand={of}\""),
        };
        write!(
            out,
            " of=\"{of}\" defs=\"{}\" count=\"{count}\" root=\"{}\" hop_tested=\"{hop_tested}\" hop_untested=\"{hop_untested}\"{} {}{next}",
            self.defs,
            self.root.xml_escaped(),
            win.xml_attrs(),
            self.gauge_attrs
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let win = self.window();
        let via_attr = if self.tag == "callees" {
            " via=\"name\""
        } else {
            ""
        };
        for it in &self.items[win.begin..win.end] {
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
                it.kind,
                it.name.xml_escaped(),
                it.path.xml_escaped(),
                it.line
            )?;
        }
        Ok(())
    }
}

fn render_nav_items<W: Write>(
    tag: &'static str,
    sym_name: &str,
    items: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    NavItemsReport {
        tag,
        root: ".",
        of: sym_name,
        defs: 1,
        items,
        gauge_attrs: crate::serialize::GraphGauges::ZERO_XML_ATTRS,
        page: crate::serialize::PageRequest::default(),
    }
    .render_envelope(out)
}

/// Serialize direct callers to Ripwire-compatible XML.
pub fn serialize_callers_xml<W: Write>(
    sym_name: &str,
    callers: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    render_nav_items("callers", sym_name, callers, out)
}

/// Serialize direct callees to Ripwire-compatible XML.
pub fn serialize_callees_xml<W: Write>(
    sym_name: &str,
    callees: &[SymbolNavInfo],
    out: &mut W,
) -> io::Result<()> {
    render_nav_items("callees", sym_name, callees, out)
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
        self.importers.iter().copied().collect()
    }
}

impl crate::serialize::XmlReport for ImpactReport<'_> {
    crate::xml_report_meta!(
        "impact",
        "ripwire-rs.impact/v1",
        root: false,
        comment: "ripwire-rs impact schema=ripwire-rs.impact/v1: transitive blast radius of of=."
    );
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
                n.kind,
                n.name.xml_escaped(),
                n.path.xml_escaped(),
                n.line,
                n.hops,
                n.hops
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
        "<find-symbol schema=\"ripwire-rs.find-symbol/v1\" of=\"{}\" p=\"{}:{}\" callers=\"{}\" callees=\"{}\">",
        nav.target.name.xml_escaped(),
        nav.target.path.xml_escaped(),
        nav.target.line,
        nav.callers.len(),
        nav.callees.len()
    )?;
    writeln!(out, "  <callers>")?;
    for c in &nav.callers {
        writeln!(
            out,
            "    <s t=\"{}\" n=\"{}\" p=\"{}:{}\"/>",
            c.kind,
            c.name.xml_escaped(),
            c.path.xml_escaped(),
            c.line
        )?;
    }
    writeln!(out, "  </callers>")?;
    writeln!(out, "  <callees>")?;
    for c in &nav.callees {
        writeln!(
            out,
            "    <s t=\"{}\" n=\"{}\" p=\"{}:{}\"/>",
            c.kind,
            c.name.xml_escaped(),
            c.path.xml_escaped(),
            c.line
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
    serialize_uses_with_gauge_xml(
        ".",
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
    crate::xml_report_meta!(
        "uses",
        "ripwire-rs.uses/v1",
        root: false,
        comment: "ripwire-rs uses schema=ripwire-rs.uses/v1: resolvable use-sites of of=."
    );
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let total = self.sites.len();
        let page_attrs = crate::serialize::PageWindow::compute(total, 40, None, None).xml_attrs();
        write!(
            out,
            " of=\"{}\" defs=\"1\" external=\"0\" count=\"{total}\" root=\"{}\"{page_attrs} {}",
            self.sym_name, self.root, self.gauge_attrs
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for &(role, file, line, enc) in self.sites.iter().take(40) {
            if enc.is_empty() {
                writeln!(
                    out,
                    "  <u role=\"{}\" p=\"{}:{}\"/>",
                    role,
                    file.xml_escaped(),
                    line
                )?;
            } else {
                writeln!(
                    out,
                    "  <u role=\"{}\" p=\"{}:{}\" in_id=\"{}\"/>",
                    role,
                    file.xml_escaped(),
                    line,
                    enc.xml_escaped()
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
    let sites = (ingest, interner)
        .collect_symbol_uses(sym_name)
        .into_iter()
        .map(|(role, file, line, enc)| (role, file, line, enc.unwrap_or("")))
        .collect();
    UsesReport {
        root,
        sym_name,
        sites,
        gauge_attrs,
    }
    .render_envelope(out)
}
