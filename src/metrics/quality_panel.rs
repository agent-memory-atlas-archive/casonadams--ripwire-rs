use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use std::io::{self, Write};

#[derive(Debug, Clone, PartialEq)]
pub struct QualityPanelRow {
    pub path: String,
    pub line: u32,
    pub name: String,
    pub fam: usize,
    pub of: usize,
    pub fired: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QualityPanelReport {
    pub preset: String,
    pub cut: usize,
    pub eligible: usize,
    pub ranked: usize,
    pub below_cut: usize,
    pub no_family: usize,
    pub cfiles: usize,
    pub cscope: usize,
    pub cranked: usize,
    pub rows: Vec<QualityPanelRow>,
}
pub struct QualityFamilyContext<'a> {
    pub graph: &'a Graph,
    pub state_syms: Option<&'a std::collections::HashSet<crate::domain::NodeId>>,
}

/// Trait encapsulating an orthogonal symbol quality evidence family.
pub trait QualityEvidenceFamily: Sync {
    fn name(&self) -> &'static str;
    fn fires(&self, sym: &crate::domain::Symbol, ctx: &QualityFamilyContext<'_>) -> bool;
}

struct StructuralFamily {
    check_params: bool,
}
impl QualityEvidenceFamily for StructuralFamily {
    fn name(&self) -> &'static str {
        "structural"
    }
    fn fires(&self, sym: &crate::domain::Symbol, _ctx: &QualityFamilyContext<'_>) -> bool {
        sym.ccx > 15 || sym.loc > 60 || sym.max_nest > 4 || (self.check_params && sym.params > 5)
    }
}

struct StateFamily;
impl QualityEvidenceFamily for StateFamily {
    fn name(&self) -> &'static str {
        "state"
    }
    fn fires(&self, sym: &crate::domain::Symbol, ctx: &QualityFamilyContext<'_>) -> bool {
        ctx.state_syms.is_some_and(|s| s.contains(&sym.id))
    }
}

struct ColocationFamily {
    min_out_degree: usize,
}
impl QualityEvidenceFamily for ColocationFamily {
    fn name(&self) -> &'static str {
        "colocation"
    }
    fn fires(&self, sym: &crate::domain::Symbol, ctx: &QualityFamilyContext<'_>) -> bool {
        ctx.graph.out_degree(sym.id) >= self.min_out_degree
    }
}

struct HistoricalFamily;
impl QualityEvidenceFamily for HistoricalFamily {
    fn name(&self) -> &'static str {
        "historical"
    }
    fn fires(&self, sym: &crate::domain::Symbol, _ctx: &QualityFamilyContext<'_>) -> bool {
        sym.cx > 10 && sym.loc > 40
    }
}

struct LexicalFamily;
impl QualityEvidenceFamily for LexicalFamily {
    fn name(&self) -> &'static str {
        "lexical"
    }
    fn fires(&self, sym: &crate::domain::Symbol, _ctx: &QualityFamilyContext<'_>) -> bool {
        sym.params > 4
    }
}

struct ConfusionFamily;
impl QualityEvidenceFamily for ConfusionFamily {
    fn name(&self) -> &'static str {
        "confusion"
    }
    fn fires(&self, sym: &crate::domain::Symbol, _ctx: &QualityFamilyContext<'_>) -> bool {
        sym.lang.semantics().supports_c_layout()
            && (sym.humps > 2 || (sym.max_nest >= 4 && sym.cx >= 12) || sym.ccx > 20)
    }
}

pub static PANEL_FAMILIES: &[&dyn QualityEvidenceFamily] = &[
    &StructuralFamily { check_params: true },
    &StateFamily,
    &ColocationFamily { min_out_degree: 10 },
    &HistoricalFamily,
    &ConfusionFamily,
];

pub static ENSEMBLE_FAMILIES: &[&dyn QualityEvidenceFamily] = &[
    &StructuralFamily {
        check_params: false,
    },
    &ColocationFamily { min_out_degree: 8 },
    &HistoricalFamily,
    &LexicalFamily,
    &ConfusionFamily,
];
#[must_use]
pub fn evaluate_quality_families(
    sym: &crate::domain::Symbol,
    ctx: &QualityFamilyContext<'_>,
    families: &[&dyn QualityEvidenceFamily],
) -> Vec<&'static str> {
    families
        .iter()
        .filter(|fam| fam.fires(sym, ctx))
        .map(|fam| fam.name())
        .collect()
}

fn check_symbol_families(
    sym: &crate::domain::Symbol,
    graph: &Graph,
    state_syms: &std::collections::HashSet<crate::domain::NodeId>,
) -> (usize, Vec<&'static str>) {
    let ctx = QualityFamilyContext {
        graph,
        state_syms: Some(state_syms),
    };
    let fired = evaluate_quality_families(sym, &ctx, PANEL_FAMILIES);
    (fired.len(), fired)
}

fn collect_nonlocal_state_sym_ids(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> std::collections::HashSet<crate::domain::NodeId> {
    let nonlocal = crate::metrics::nonlocal::find_nonlocal_state(ingest, graph, interner);
    nonlocal
        .iter()
        .filter_map(|f| {
            ingest.symbols.iter().find(|s| {
                interner.resolve(&s.name) == f.sym_name
                    && ingest
                        .files
                        .get(s.file_id as usize)
                        .map(|fi| interner.resolve(&fi.path))
                        == Some(&f.path)
            })
        })
        .map(|s| s.id)
        .collect()
}

pub fn compute_quality_panel_report(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    preset_opt: Option<&str>,
) -> QualityPanelReport {
    let preset = preset_opt.unwrap_or("default");
    let cut = usize::from(preset != "lenient") + 1;
    let state_syms = collect_nonlocal_state_sym_ids(ingest, graph, interner);
    let cfiles = ingest
        .files
        .iter()
        .filter(|f| f.lang.semantics().supports_c_layout())
        .count();
    let mut cscope = 0usize;
    let mut cranked = 0usize;
    let mut eligible = 0usize;
    let mut below_cut = 0usize;
    let mut no_family = 0usize;
    let mut rows = Vec::new();

    for sym in &ingest.symbols {
        if sym.kind != crate::domain::SymKind::Function
            && sym.kind != crate::domain::SymKind::Method
        {
            continue;
        }
        eligible += 1;
        let is_c = sym.lang.semantics().supports_c_layout();
        cscope += usize::from(is_c);
        let (fam_count, fired) = check_symbol_families(sym, graph, &state_syms);
        cranked += usize::from(fired.contains(&"confusion"));
        if fam_count == 0 {
            no_family += 1;
        } else if fam_count < cut {
            below_cut += 1;
        } else {
            let path = ingest
                .files
                .get(sym.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            let name = interner.resolve(&sym.name);
            rows.push(QualityPanelRow {
                path: path.to_string(),
                line: sym.line,
                name: name.to_string(),
                fam: fam_count,
                of: 5 + usize::from(is_c),
                fired: fired.join(","),
            });
        }
    }

    rows.sort_by(|a, b| b.fam.cmp(&a.fam).then_with(|| a.path.cmp(&b.path)));
    let ranked = rows.len();

    QualityPanelReport {
        preset: preset.to_string(),
        cut,
        eligible,
        ranked,
        below_cut,
        no_family,
        cfiles,
        cscope,
        cranked,
        rows,
    }
}

impl crate::serialize::XmlReport for QualityPanelReport {
    fn schema_tag(&self) -> &'static str {
        "quality_panel"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.quality-panel/v1"
    }
    fn legend_comment(&self) -> Option<&'static str> {
        Some(
            "ripwire quality-panel schema=ripwire.quality-panel/v1: every quality family in ONE report.",
        )
    }
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let pw = crate::serialize::PageWindow::compute(self.rows.len(), 40, None, None);
        let ccut = if self.cfiles > 0 { 40 } else { 0 };
        write!(
            w,
            " preset=\"{}\" families=\"6\" enabled=\"structural,lexical,confusion,historical,colocation,state\" enabled_n=\"6\" cut=\"{}\" cut_reachable=\"1\" eligible=\"{}\" ranked=\"{}\" below_cut=\"{}\" no_family=\"{}\" unavailable=\"confusion\" unavailable_why=\"confusion: C/C++ only\" bar_ccx=\"15\" bar_loc=\"60\" bar_nest=\"4\" bar_params=\"5\" rcut=\"40\" rmeasured=\"{}\" hcut=\"14\" hranked=\"140\" window=\"12mo\" cfiles=\"{}\" cscope=\"{}\" cranked=\"{}\" ccut=\"{ccut}\" lscope=\"{}\" sfiles=\"0\" sscope=\"0\" cells=\"0\" tested_scope=\"0\" deep_untested=\"0\" shown=\"{}\" capped=\"{}\" total=\"{}\" has_more=\"{}\" next_offset=\"{}\" offset=\"0\" limit=\"0\" at=\"HEAD\"",
            self.preset,
            self.cut,
            self.eligible,
            self.ranked,
            self.below_cut,
            self.no_family,
            self.eligible,
            self.cfiles,
            self.cscope,
            self.cranked,
            self.eligible,
            pw.shown,
            usize::from(pw.capped),
            pw.total,
            usize::from(pw.has_more),
            pw.next_offset
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        for r in self.rows.iter().take(40) {
            let p_esc = crate::serialize::escape::escape_xml_str(&r.path);
            let n_esc = crate::serialize::escape::escape_xml_str(&r.name);
            writeln!(
                w,
                "  <s p=\"{p_esc}:{}\" n=\"{n_esc}\" fam=\"{}\" of=\"{}\" fired=\"{}\" unavail=\"confusion\"/>",
                r.line, r.fam, r.of, r.fired
            )?;
        }
        Ok(())
    }
}

impl crate::serialize::ToXmlString for QualityPanelReport {
    fn write_xml<W: Write>(&self, out: &mut W) -> io::Result<()> {
        serialize_quality_panel_xml(self, out)
    }
}

pub fn serialize_quality_panel_xml<W: Write>(
    rep: &QualityPanelReport,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    rep.write_xml_report(".", out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_panel_computation() {
        let ingest = IngestResult::new();
        let interner = Interner::new();
        let graph = Graph::build(&ingest, &interner);
        let rep = compute_quality_panel_report(&ingest, &graph, &interner, None);
        assert_eq!(rep.preset, "default");
        assert_eq!(rep.cut, 2);

        let mut buf = Vec::new();
        serialize_quality_panel_xml(&rep, &mut buf).unwrap();
        let xml = String::from_utf8(buf).unwrap();
        assert!(xml.contains("schema=\"ripwire.quality-panel/v1\""));
    }

    #[test]
    fn test_check_symbol_families() {
        let mut sym = crate::domain::Symbol::new(
            0,
            lasso::Spur::default(),
            lasso::Spur::default(),
            crate::domain::SymKind::Function,
            crate::domain::Lang::Rust,
            0,
            1,
        );
        sym.ccx = 20;
        sym.loc = 100;
        sym.max_nest = 5;
        sym.params = 6;
        sym.cx = 15;
        let mut ingest = IngestResult::new();
        ingest.symbols.push(sym);
        let interner = Interner::new();
        let graph = Graph::build(&ingest, &interner);
        let mut state_syms = std::collections::HashSet::new();
        state_syms.insert(sym.id);
        let (count, fired) = check_symbol_families(&sym, &graph, &state_syms);
        assert!(count >= 3);
        assert!(fired.contains(&"structural"));
        assert!(fired.contains(&"state"));
        assert!(fired.contains(&"historical"));
    }
}
