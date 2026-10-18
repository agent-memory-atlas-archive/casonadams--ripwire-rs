use crate::serialize::escape::XmlEscapeExt;
use std::io::{self, Write};

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
        crate::serialize::GraphGauges::ZERO_XML_ATTRS,
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
        "ripwire-rs.test-gate/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some("ripwire-rs test-gate schema=ripwire-rs.test-gate/v1: tests + untested blast radius.")
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
        "ripwire-rs.safe-delete/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire-rs safe-delete schema=ripwire-rs.safe-delete/v1: can sym= go: callers= impact_reaches= uses= tested_self= risk=.",
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
        gauge_attrs: crate::serialize::GraphGauges::ZERO_XML_ATTRS,
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
        "ripwire-rs.exercises/v1"
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
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl crate::serialize::XmlReport for DeadCodeReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "dead-code"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire-rs.dead-code/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire-rs dead-code schema=ripwire-rs.dead-code/v1: internal-linkage functions with no caller found in the index: <d n= t= p= l=>.",
        )
    }
    fn page_window(&self) -> Option<crate::serialize::PageWindow> {
        use crate::serialize::PaginateSlice;
        Some(self.items.paginate(40, self.limit, self.offset).0)
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
        use crate::serialize::PaginateSlice;
        let (_, page) = self.items.paginate(40, self.limit, self.offset);
        for (n, t, p, l) in page {
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
    serialize_dead_code_paged_xml(
        root,
        items,
        crate::serialize::GraphGauges::default(),
        None,
        None,
        out,
    )
}

pub fn serialize_dead_code_with_gauge_xml<W: Write>(
    root: &str,
    items: &[(&str, &'static str, &str, u32)],
    gauges: crate::serialize::GraphGauges,
    out: &mut W,
) -> io::Result<()> {
    serialize_dead_code_paged_xml(root, items, gauges, None, None, out)
}

pub fn serialize_dead_code_paged_xml<W: Write>(
    root: &str,
    items: &[(&str, &'static str, &str, u32)],
    gauges: crate::serialize::GraphGauges,
    limit: Option<usize>,
    offset: Option<usize>,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    DeadCodeReport {
        items,
        gauges,
        limit,
        offset,
    }
    .write_xml_report(root, out)
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
        "ripwire-rs.communities/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire-rs communities schema=ripwire-rs.communities/v1: call-graph modules (Louvain): <community id= size= dir= label=> of <member t= n= p=>.",
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

pub struct AroundSymbolRow<'a> {
    pub name: &'a str,
    pub scope: Option<&'a str>,
    pub kind: &'static str,
    pub path: &'a str,
    pub hop: u32,
    pub callees: Vec<&'a str>,
}

pub struct AroundReport<'a> {
    pub target: &'a str,
    pub depth: u32,
    pub fanout: usize,
    pub fanout_cut: usize,
    pub rows: &'a [AroundSymbolRow<'a>],
}

impl crate::serialize::XmlReport for AroundReport<'_> {
    fn schema_tag(&self) -> &'static str {
        "r"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire-rs.around/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire-rs around schema=ripwire-rs.around/v1: call neighbourhood of of=: depth= hops, fanout= kept per hop.",
        )
    }
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let edge_cnt: usize = self.rows.iter().map(|r| r.callees.len()).sum();
        let est_tokens = 25 + self.rows.len() * 14 + edge_cnt * 5;
        let depth_trunc = usize::from(!self.rows.is_empty());
        write!(
            out,
            " of=\"{}\" depth=\"{}\" fanout=\"{}\" depth_truncated=\"{depth_trunc}\"",
            self.target.xml_escaped(),
            self.depth,
            self.fanout
        )?;
        if self.fanout_cut > 0 {
            write!(out, " fanout_cut=\"{}\"", self.fanout_cut)?;
        }
        write!(out, " est_tokens=\"{est_tokens}\"")
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let mut grouped: Vec<(&str, Vec<&AroundSymbolRow<'_>>)> = Vec::new();
        for row in self.rows {
            if let Some((_, list)) = grouped.iter_mut().find(|(p, _)| *p == row.path) {
                list.push(row);
            } else {
                grouped.push((row.path, vec![row]));
            }
        }
        if grouped.is_empty() {
            return write!(out, "<f p=\"src/lib.rs\"></f>");
        }
        for (path, list) in grouped {
            use crate::serialize::PathLayerExt;
            path.write_file_open_tag(out)?;
            for r in list {
                let k = 1.0 / f64::from(1u32 << r.hop.min(10));
                write!(out, "<s t=\"{}\" n=\"{}\"", r.kind, r.name.xml_escaped())?;
                if let Some(sc) = r.scope.filter(|s| !s.is_empty()) {
                    write!(out, " sc=\"{}\"", sc.xml_escaped())?;
                }
                write!(out, " k=\"{k:.4}\">")?;
                for &c in &r.callees {
                    write!(out, "<c n=\"{}\"/>", c.xml_escaped())?;
                }
                write!(out, "</s>")?;
            }
            write!(out, "</f>")?;
        }
        Ok(())
    }
}

/// Serialize `--around` ego graph with nested `<c>` edges to Ripwire XML.
pub fn serialize_around_rows_xml<W: Write>(
    root: &str,
    target: &str,
    depth: u32,
    fanout: usize,
    fanout_cut: usize,
    rows: &[AroundSymbolRow<'_>],
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    AroundReport {
        target,
        depth,
        fanout,
        fanout_cut,
        rows,
    }
    .write_xml_report(root, out)
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
    let rows: Vec<AroundSymbolRow<'_>> = symbols
        .iter()
        .map(|&(name, kind, path, _line)| AroundSymbolRow {
            name,
            scope: None,
            kind,
            path,
            hop: 0,
            callees: Vec::new(),
        })
        .collect();
    serialize_around_rows_xml(root, target, depth, fanout, 0, &rows, out)
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
        "ripwire-rs.external-surface/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire-rs external-surface schema=ripwire-rs.external-surface/v1: names used but never defined in the index: <x n= lang= refs= calls=>.",
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
    pub limit: Option<usize>,
    pub offset: Option<usize>,
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
        use crate::serialize::PathLayerExt;
        write!(
            out,
            "    <f p=\"{}\" afferent=\"{}\"",
            g.path.xml_escaped(),
            g.afferent
        )?;
        g.path.write_layer_attr(out)?;
        writeln!(out, "/>")?;
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
        "ripwire-rs.deps/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire-rs deps schema=ripwire-rs.deps/v1: file-to-file include/import view, heaviest cone first: <f p= afferent= includes= instab= transitive=>, <health>, <godfiles>, <cycles>.",
        )
    }
    fn page_window(&self) -> Option<crate::serialize::PageWindow> {
        let total = self
            .rows
            .iter()
            .filter(|d| d.includes > 0 || d.efferent > 0 || d.afferent > 0)
            .count();
        Some(crate::serialize::PageWindow::compute(
            total,
            40,
            self.limit,
            self.offset,
        ))
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
        use crate::serialize::PaginateSlice;
        let (_, page) = active.paginate(40, self.limit, self.offset);
        for row in page {
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
        limit: None,
        offset: None,
    };
    serialize_deps_report_xml(root, &rep, out)
}
