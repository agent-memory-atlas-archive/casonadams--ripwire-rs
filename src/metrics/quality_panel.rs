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
    pub hranked: usize,
    pub cfiles: usize,
    pub cscope: usize,
    pub cranked: usize,
    pub sfiles: usize,
    pub sscope: usize,
    pub cells: usize,
    pub tested_scope: usize,
    pub deep_untested: usize,
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

pub struct QualityFamilySpec {
    pub name: &'static str,
    pub predicate: fn(&crate::domain::Symbol, &QualityFamilyContext<'_>) -> bool,
}

impl QualityEvidenceFamily for QualityFamilySpec {
    fn name(&self) -> &'static str {
        self.name
    }
    fn fires(&self, sym: &crate::domain::Symbol, ctx: &QualityFamilyContext<'_>) -> bool {
        (self.predicate)(sym, ctx)
    }
}

fn is_structural_base(sym: &crate::domain::Symbol) -> bool {
    sym.ccx > 15 || sym.loc > 60 || sym.max_nest > 4
}

const HISTORICAL_FAMILY: QualityFamilySpec = QualityFamilySpec {
    name: "historical",
    predicate: |sym, _| sym.cx > 10 && sym.loc > 40,
};

const CONFUSION_FAMILY: QualityFamilySpec = QualityFamilySpec {
    name: "confusion",
    predicate: |sym, _| {
        sym.lang.semantics().supports_c_layout()
            && (sym.humps > 2 || (sym.max_nest >= 4 && sym.cx >= 12) || sym.ccx > 20)
    },
};

pub static PANEL_FAMILIES: &[&dyn QualityEvidenceFamily] = &[
    &QualityFamilySpec {
        name: "structural",
        predicate: |sym, _| is_structural_base(sym) || sym.params > 5,
    },
    &QualityFamilySpec {
        name: "state",
        predicate: |sym, ctx| ctx.state_syms.is_some_and(|s| s.contains(&sym.id)),
    },
    &QualityFamilySpec {
        name: "colocation",
        predicate: |sym, ctx| ctx.graph.out_degree(sym.id) >= 10,
    },
    &HISTORICAL_FAMILY,
    &CONFUSION_FAMILY,
];

pub static ENSEMBLE_FAMILIES: &[&dyn QualityEvidenceFamily] = &[
    &QualityFamilySpec {
        name: "structural",
        predicate: |sym, _| is_structural_base(sym),
    },
    &QualityFamilySpec {
        name: "colocation",
        predicate: |sym, ctx| ctx.graph.out_degree(sym.id) >= 8,
    },
    &HISTORICAL_FAMILY,
    &QualityFamilySpec {
        name: "lexical",
        predicate: |sym, _| sym.params > 4,
    },
    &CONFUSION_FAMILY,
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

fn collect_nonlocal_state_info(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> (
    std::collections::HashSet<crate::domain::NodeId>,
    usize,
    usize,
) {
    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let nonlocal = crate::metrics::nonlocal::find_nonlocal_state(ingest, graph, interner);
    let cells = nonlocal.len();
    let mut state_files = std::collections::HashSet::new();
    let mut state_syms = std::collections::HashSet::new();
    for f in &nonlocal {
        state_files.insert(f.path.as_str());
        if let Some(s) = view.find_symbol_with_hint(&f.sym_name, Some(&f.path)) {
            state_syms.insert(s.id);
        }
    }
    (state_syms, state_files.len(), cells)
}

pub fn compute_quality_panel_report(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    preset_opt: Option<&str>,
) -> QualityPanelReport {
    let preset = preset_opt.unwrap_or("default");
    let cut = usize::from(preset != "lenient") + 1;
    let (state_syms, sfiles, cells) = collect_nonlocal_state_info(ingest, graph, interner);
    let sscope = state_syms.len();
    let tested_mask = crate::navigate::compute_tested_node_mask(ingest, graph, interner);
    let cfiles = ingest
        .files
        .iter()
        .filter(|f| f.lang.semantics().supports_c_layout())
        .count();
    let mut cscope = 0usize;
    let mut cranked = 0usize;
    let mut hranked = 0usize;
    let mut tested_scope = 0usize;
    let mut deep_untested = 0usize;
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
        let is_tested = tested_mask.get(sym.id as usize).copied().unwrap_or(false);
        tested_scope += usize::from(is_tested);
        deep_untested += usize::from(!is_tested && sym.ccx > 15);
        let is_c = sym.lang.semantics().supports_c_layout();
        cscope += usize::from(is_c);
        let (fam_count, fired) = check_symbol_families(sym, graph, &state_syms);
        cranked += usize::from(fired.contains(&"confusion"));
        hranked += usize::from(fired.contains(&"historical"));
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
        hranked,
        cfiles,
        cscope,
        cranked,
        sfiles,
        sscope,
        cells,
        tested_scope,
        deep_untested,
        rows,
    }
}

impl crate::serialize::XmlReport for QualityPanelReport {
    crate::xml_report_meta!(
        "quality_panel",
        "ripwire-rs.quality-panel/v1",
        root: true,
        comment: "ripwire-rs quality-panel schema=ripwire-rs.quality-panel/v1: every quality family in ONE report."
    );
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let pw = crate::serialize::PageWindow::compute(self.rows.len(), 40, None, None);
        let ccut = if self.cfiles > 0 { 40 } else { 0 };
        let unavail_attrs = if self.cfiles == 0 {
            " unavailable=\"confusion\" unavailable_why=\"confusion: C/C++ only\""
        } else {
            ""
        };
        write!(
            w,
            " preset=\"{}\" families=\"6\" enabled=\"structural,lexical,confusion,historical,colocation,state\" enabled_n=\"6\" cut=\"{}\" cut_reachable=\"1\" eligible=\"{}\" ranked=\"{}\" below_cut=\"{}\" no_family=\"{}\"{unavail_attrs} bar_ccx=\"15\" bar_loc=\"60\" bar_nest=\"4\" bar_params=\"5\" rcut=\"40\" rmeasured=\"{}\" hcut=\"14\" hranked=\"{}\" window=\"12mo\" cfiles=\"{}\" cscope=\"{}\" cranked=\"{}\" ccut=\"{ccut}\" lscope=\"{}\" sfiles=\"{}\" sscope=\"{}\" cells=\"{}\" tested_scope=\"{}\" deep_untested=\"{}\" shown=\"{}\" capped=\"{}\" total=\"{}\" has_more=\"{}\" next_offset=\"{}\" offset=\"0\" limit=\"0\" at=\"HEAD\"",
            self.preset,
            self.cut,
            self.eligible,
            self.ranked,
            self.below_cut,
            self.no_family,
            self.eligible,
            self.hranked,
            self.cfiles,
            self.cscope,
            self.cranked,
            self.eligible,
            self.sfiles,
            self.sscope,
            self.cells,
            self.tested_scope,
            self.deep_untested,
            pw.shown,
            usize::from(pw.capped),
            pw.total,
            usize::from(pw.has_more),
            pw.next_offset
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let unavail_row_attr = if self.cfiles == 0 {
            " unavail=\"confusion\""
        } else {
            ""
        };
        for r in self.rows.iter().take(40) {
            use crate::serialize::XmlEscapeExt;
            writeln!(
                w,
                "  <s p=\"{}:{}\" n=\"{}\" fam=\"{}\" of=\"{}\" fired=\"{}\"{unavail_row_attr}/>",
                r.path.xml_escaped(),
                r.line,
                r.name.xml_escaped(),
                r.fam,
                r.of,
                r.fired
            )?;
        }
        Ok(())
    }
}

pub fn serialize_quality_panel_xml<W: Write>(
    rep: &QualityPanelReport,
    out: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlReport;
    rep.write_xml_report(".", out)
}
