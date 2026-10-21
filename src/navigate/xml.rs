use super::{DeclinedCall, ImpactDef, ImpactNode, NavNeighborhood, SymbolNavInfo};
use crate::domain::{IngestResult, Interner, SymKind};
use crate::serialize::XmlEscapeExt;
use std::io::{self, Write};

/// Direction of a 1-hop navigation report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDir {
    Callers,
    Callees,
}

impl NavDir {
    fn tag(self) -> &'static str {
        match self {
            Self::Callers => "callers",
            Self::Callees => "callees",
        }
    }
}

pub struct NavItemsReport<'a> {
    pub dir: NavDir,
    pub root: &'a str,
    pub of: &'a str,
    pub defs: usize,
    pub def_rows: &'a [ImpactDef<'a>],
    pub items: &'a [SymbolNavInfo],
    pub declined: &'a [DeclinedCall<'a>],
    pub gauge_attrs: &'a str,
    pub page: crate::serialize::PageRequest,
}

impl NavItemsReport<'_> {
    fn window(&self) -> crate::serialize::PageWindow {
        self.page.window(self.items.len(), 40)
    }

    fn grouped(&self) -> bool {
        self.dir == NavDir::Callers && self.def_rows.len() >= 2
    }

    fn of_def_attr(&self, item: &SymbolNavInfo) -> String {
        if !self.grouped() {
            return String::new();
        }
        let ixs: Vec<String> = self
            .def_rows
            .iter()
            .enumerate()
            .filter_map(|(ix, d)| {
                let (_, prov) = item.of_defs.iter().find(|(id, _)| d.members.contains(id))?;
                Some(format!("{}{}", ix + 1, prov.of_def_suffix()))
            })
            .collect();
        format!(" of_def=\"{}\"", ixs.join(" "))
    }

    fn of_all_defs_attr(&self) -> String {
        if !self.grouped() {
            return String::new();
        }
        let ixs: Vec<String> = (1..=self.def_rows.len())
            .map(|ix| format!("{ix}~d"))
            .collect();
        format!(" of_def=\"{}\"", ixs.join(" "))
    }

    fn declined_attr(&self) -> String {
        match self.declined.len() {
            0 => String::new(),
            n => format!(" declined_calls=\"{n}\""),
        }
    }

    /// `role=`, `prov=`, and `via=` for one row: how the edge to or from it bound.
    fn binding_attrs(&self, it: &SymbolNavInfo) -> String {
        let prov = match self.dir {
            NavDir::Callees => it.prov,
            NavDir::Callers => it.of_defs.iter().map(|(_, p)| *p).max().unwrap_or(it.prov),
        };
        let role = if self.dir == NavDir::Callees && it.kind == SymKind::Macro.tag() {
            " role=\"macro\""
        } else {
            ""
        };
        match (self.dir, prov.callee_label()) {
            (NavDir::Callees, Some(label)) => format!("{role} prov=\"{label}\""),
            _ if prov.is_by_name() => format!("{role} via=\"name\""),
            _ => role.to_string(),
        }
    }
}

fn write_def_rows<W: Write>(out: &mut W, defs: &[ImpactDef<'_>]) -> io::Result<()> {
    if defs.len() < 2 {
        return Ok(());
    }
    for (ix, d) in defs.iter().enumerate() {
        let sc = d
            .scope
            .map(|s| format!(" sc=\"{}\"", s.xml_escaped()))
            .unwrap_or_default();
        write!(
            out,
            "<def ix=\"{}\" t=\"{}\"{sc} p=\"{}:{}\"{}/>",
            ix + 1,
            d.kind,
            d.path.xml_escaped(),
            d.line,
            if d.bodyless { " bodyless=\"1\"" } else { "" }
        )?;
    }
    Ok(())
}

fn write_declined_row<W: Write>(
    out: &mut W,
    dc: &DeclinedCall<'_>,
    dir: Option<NavDir>,
    of_def: &str,
) -> io::Result<()> {
    match dir {
        Some(NavDir::Callees) => write!(
            out,
            "<dc n=\"{}\" l=\"{}\" cands=\"{}\" via=\"declined\"/>",
            dc.callee.xml_escaped(),
            dc.line,
            dc.cands
        ),
        _ => write!(
            out,
            "<dc in=\"{}\" p=\"{}:{}\" cands=\"{}\" via=\"declined\"{of_def}/>",
            dc.enclosing.xml_escaped(),
            dc.path.xml_escaped(),
            dc.line,
            dc.cands
        ),
    }
}

impl crate::serialize::XmlReport for NavItemsReport<'_> {
    fn schema_tag(&self) -> &'static str {
        self.dir.tag()
    }
    fn schema_version(&self) -> &'static str {
        match self.dir {
            NavDir::Callers => "ripwire-rs.callers/v1",
            NavDir::Callees => "ripwire-rs.callees/v1",
        }
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(match self.dir {
            NavDir::Callers => {
                "ripwire-rs callers schema=ripwire-rs.callers/v1: 1-hop callers of of=. <def ix= t= sc= p= bodyless=1>: the groups defs= forms (a definition and the declarations tied to it; bodyless=1 tied to none), listed when there are two or more. of_def=K[~s|~t|~n]: the group the caller edge reaches (~s: one arm of an ambiguous call; ~t: a primary template whose specialization is another group; ~n: bound by name alone). via=name: the edge bound by name alone. declined_calls=K: call sites naming of= the resolver left unbound, in no count; <dc in= p= cands= via=declined>: one such site (calling def, file:line), NOT an edge; cands=K: in-tree defs it could have meant."
            }
            NavDir::Callees => {
                "ripwire-rs callees schema=ripwire-rs.callees/v1: 1-hop callees of of=. role=macro: the callee is a macro expansion. via=name: bound by name alone (same directory or repository-wide). prov=scip|binding|import|final-segment|split: how the edge bound; scip = pinned by a --scip index; binding = exposed to the caller's language by a binding framework (pybind11 .def); import = through a module alias the file imports; final-segment = a type qualifier matched by last name only; split = one arm of an ambiguous call. declined_calls=K: call sites inside of= the resolver left unbound, in no count; <dc n= l= cands= via=declined>: one such site (called name, line), NOT an edge; cands=K: in-tree defs it could have meant."
            }
        })
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
        let tag = self.dir.tag();
        let uses_next = format!(" uses_next=\"--uses={of}\"");
        let next = match (win.has_more, self.dir) {
            (true, dir) => format!(
                " next=\"--{tag}={of} --offset={}\"{}",
                win.next_offset,
                if dir == NavDir::Callers {
                    uses_next.as_str()
                } else {
                    ""
                }
            ),
            (false, NavDir::Callers) => format!(" next=\"--uses={of}\""),
            (false, NavDir::Callees) => format!(" next=\"--expand={of}\""),
        };
        write!(
            out,
            " of=\"{of}\" defs=\"{}\" count=\"{count}\" root=\"{}\" hop_tested=\"{hop_tested}\" hop_untested=\"{hop_untested}\"{}{} {}{next}",
            self.defs,
            self.root.xml_escaped(),
            self.declined_attr(),
            win.xml_attrs(),
            self.gauge_attrs
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let win = self.window();
        if self.grouped() {
            write_def_rows(out, self.def_rows)?;
        }
        for it in &self.items[win.begin..win.end] {
            let binding_attrs = self.binding_attrs(it);
            let tested_attr = if it.tested || crate::gate::affected::is_test_path(&it.path) {
                " tested=\"1\""
            } else {
                ""
            };
            let sites_attr = if self.dir == NavDir::Callers && !it.sites.is_empty() {
                format!(" sites_at=\"{}\"", it.sites.join(" ").xml_escaped())
            } else {
                String::new()
            };
            write!(
                out,
                "<s t=\"{}\" n=\"{}\" p=\"{}:{}\"{binding_attrs}{}{tested_attr}{sites_attr}/>",
                it.kind,
                it.name.xml_escaped(),
                it.path.xml_escaped(),
                it.line,
                self.of_def_attr(it)
            )?;
        }
        let all_defs = self.of_all_defs_attr();
        for dc in self.declined {
            write_declined_row(out, dc, Some(self.dir), &all_defs)?;
        }
        Ok(())
    }
}

pub struct ImpactReport<'a> {
    pub root: &'a str,
    pub sym_name: &'a str,
    pub defs: Vec<ImpactDef<'a>>,
    pub nodes: &'a [ImpactNode],
    pub importers: Vec<&'a str>,
    pub declined: Vec<DeclinedCall<'a>>,
    pub gauge_attrs: &'a str,
    pub pr_iters: u32,
    pub page: crate::serialize::PageRequest,
}

const IMPACT_ROWS_CAP: usize = 40;
const IMPACT_DC_CAP: usize = 16;
const IMPACT_FILES_CAP: usize = 20;

impl ImpactReport<'_> {
    fn window(&self) -> crate::serialize::PageWindow {
        self.page.window(self.nodes.len(), IMPACT_ROWS_CAP)
    }

    fn cap(&self, default_cap: usize) -> usize {
        self.page.limit.filter(|&l| l > 0).unwrap_or(default_cap)
    }

    fn shown_importers(&self) -> usize {
        self.importers.len().min(self.cap(IMPACT_ROWS_CAP))
    }

    fn shown_declined(&self) -> usize {
        self.declined.len().min(self.cap(IMPACT_DC_CAP))
    }

    fn follow_up(&self, attr: &str, limit: usize) -> String {
        format!(
            " {attr}=\"--impact={} --limit={limit}\"",
            self.sym_name.xml_escaped()
        )
    }

    fn by_depth_attr(&self) -> String {
        let mut depth_counts: std::collections::BTreeMap<u32, usize> =
            std::collections::BTreeMap::new();
        for n in self.nodes {
            *depth_counts.entry(n.hops).or_insert(0) += 1;
        }
        if depth_counts.is_empty() {
            return String::new();
        }
        let by_depth = depth_counts
            .iter()
            .map(|(d, c)| format!("{d}:{c}"))
            .collect::<Vec<_>>()
            .join(",");
        format!(" by_depth=\"{by_depth}\"")
    }

    fn importer_attrs(&self) -> String {
        let total = self.importers.len();
        let shown = self.shown_importers();
        let capped = shown < total;
        let next = if capped {
            self.follow_up("importers_next", total)
        } else {
            String::new()
        };
        format!(
            " importers=\"{total}\" shown_importers=\"{shown}\" importers_capped=\"{}\"{next}",
            usize::from(capped)
        )
    }

    fn declined_attrs(&self) -> String {
        let total = self.declined.len();
        if total == 0 {
            return String::new();
        }
        let shown = self.shown_declined();
        if shown == total {
            return format!(" declined_calls=\"{total}\"");
        }
        let next = self.follow_up("dc_next", total.max(self.cap(IMPACT_ROWS_CAP)));
        format!(" declined_calls=\"{total}\" dc_shown=\"{shown}\"{next}")
    }

    fn write_files_block<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let files = super::reach_files(self.nodes);
        let total = files.len();
        let shown = total.min(self.cap(IMPACT_FILES_CAP));
        let next = if shown < total {
            self.follow_up("files_next", total)
        } else {
            String::new()
        };
        write!(
            out,
            "<files files=\"{total}\" shown_files=\"{shown}\" files_capped=\"{}\"{next}>",
            usize::from(shown < total)
        )?;
        for f in &files[..shown] {
            write!(
                out,
                "<rf p=\"{}\" syms=\"{}\" d=\"{}\"/>",
                f.path.xml_escaped(),
                f.syms,
                f.hops
            )?;
        }
        write!(out, "</files>")
    }

    fn write_reach_rows<W: Write>(
        &self,
        win: crate::serialize::PageWindow,
        out: &mut W,
    ) -> io::Result<()> {
        let mut prev_hops = None;
        for n in &self.nodes[win.begin..win.end] {
            let depth_attr = if prev_hops == Some(n.hops) {
                String::new()
            } else {
                format!(" d=\"{}\"", n.hops)
            };
            prev_hops = Some(n.hops);
            let tested_attr = if n.tested || crate::gate::affected::is_test_path(&n.path) {
                " tested=\"1\""
            } else {
                ""
            };
            write!(
                out,
                "<s t=\"{}\" n=\"{}\" p=\"{}:{}\"{depth_attr}{tested_attr}/>",
                n.kind,
                n.name.xml_escaped(),
                n.path.xml_escaped(),
                n.line
            )?;
        }
        Ok(())
    }
}

impl crate::serialize::XmlReport for ImpactReport<'_> {
    crate::xml_report_meta!(
        "impact",
        "ripwire-rs.impact/v1",
        root: false,
        comment: "ripwire-rs impact schema=ripwire-rs.impact/v1: transitive blast radius of of=. declined_calls=K: call sites naming a reached def the resolver left unbound; <dc in= p= cands=>: one such site, NOT an edge; dc_shown=N rows printed (file-first); dc_next= lists every site."
    );
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let reaches = self.nodes.len();
        let radius_tested = self
            .nodes
            .iter()
            .filter(|n| n.tested || crate::gate::affected::is_test_path(&n.path))
            .count();
        let radius_untested = reaches.saturating_sub(radius_tested);
        let of = self.sym_name.xml_escaped();
        write!(
            out,
            " of=\"{of}\" defs=\"{}\" reaches=\"{reaches}\"{}{} radius_tested=\"{radius_tested}\" radius_untested=\"{radius_untested}\"{} root=\"{}\"{} {} pr_iters=\"{}\" next=\"--safe-delete={of}\"",
            self.defs.iter().map(|d| d.members.len()).sum::<usize>(),
            self.by_depth_attr(),
            self.importer_attrs(),
            self.declined_attrs(),
            self.root.xml_escaped(),
            self.window().xml_attrs(),
            self.gauge_attrs,
            self.pr_iters.max(1),
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let win = self.window();
        write_def_rows(out, &self.defs)?;
        if win.capped {
            self.write_files_block(out)?;
        }
        self.write_reach_rows(win, out)?;
        for p in &self.importers[..self.shown_importers()] {
            write!(
                out,
                "<f via=\"import\" p=\"{}\" lazy=\"0\"/>",
                p.xml_escaped()
            )?;
        }
        for dc in &self.declined[..self.shown_declined()] {
            write_declined_row(out, dc, None, "")?;
        }
        Ok(())
    }
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
    use crate::serialize::XmlReport;
    UsesReport::collect(sym_name, ingest, interner).render_envelope(out)
}

pub struct UsesReport<'a> {
    pub root: &'a str,
    pub sym_name: &'a str,
    pub defs: usize,
    pub sites: Vec<(&'static str, &'a str, u32, Option<&'a str>)>,
    pub gauge_attrs: &'a str,
    pub page: crate::serialize::PageRequest,
}

impl<'a> UsesReport<'a> {
    #[must_use]
    pub fn collect(sym_name: &'a str, ingest: &'a IngestResult, interner: &'a Interner) -> Self {
        use crate::domain::CorpusView;
        let mut sites = (ingest, interner).collect_symbol_uses(sym_name);
        sites.sort_by(|a, b| (a.1, a.2).cmp(&(b.1, b.2)));
        Self {
            root: ".",
            sym_name,
            defs: super::find_symbols_by_name(sym_name, ingest, interner).len(),
            sites,
            gauge_attrs: crate::serialize::GraphGauges::ZERO_XML_ATTRS,
            page: crate::serialize::PageRequest::default(),
        }
    }

    fn window(&self) -> crate::serialize::PageWindow {
        self.page.window(self.sites.len(), 0)
    }
}

impl crate::serialize::XmlReport for UsesReport<'_> {
    crate::xml_report_meta!(
        "uses",
        "ripwire-rs.uses/v1",
        root: false,
        comment: "ripwire-rs uses schema=ripwire-rs.uses/v1: resolvable use-sites of of=. defs=N: definitions the selector matched. external=1: of= has no indexed definition. limit=/offset= page the rows; next_offset= continues."
    );
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let win = self.window();
        let page_attrs = if win.limit > 0 || win.offset > 0 {
            win.xml_attrs()
        } else {
            String::new()
        };
        write!(
            out,
            " of=\"{}\" defs=\"{}\" external=\"{}\" count=\"{}\" root=\"{}\"{page_attrs} {}",
            self.sym_name.xml_escaped(),
            self.defs,
            usize::from(self.defs == 0),
            win.total,
            self.root.xml_escaped(),
            self.gauge_attrs
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let win = self.window();
        for &(role, file, line, enc) in &self.sites[win.begin..win.end] {
            let in_id = enc
                .map(|e| format!(" in_id=\"{}\"", e.xml_escaped()))
                .unwrap_or_default();
            writeln!(
                out,
                "  <u role=\"{role}\" p=\"{}:{line}\"{in_id}/>",
                file.xml_escaped()
            )?;
        }
        Ok(())
    }
}
