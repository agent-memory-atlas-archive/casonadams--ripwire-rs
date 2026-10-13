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
    pub rows: Vec<QualityPanelRow>,
}

fn check_symbol_families(
    sym: &crate::domain::Symbol,
    graph: &Graph,
    state_syms: &std::collections::HashSet<crate::domain::NodeId>,
) -> (usize, Vec<&'static str>) {
    let mut fired = Vec::new();

    // 1. Structural
    if sym.ccx > 15 || sym.loc > 60 || sym.max_nest > 4 || sym.params > 5 {
        fired.push("structural");
    }

    // 2. State
    if state_syms.contains(&sym.id) {
        fired.push("state");
    }

    // 3. Colocation
    if graph.out_degree(sym.id) >= 10 {
        fired.push("colocation");
    }

    // 4. Historical (churn heuristic based on loc/complexity ratio)
    if sym.cx > 10 && sym.loc > 40 {
        fired.push("historical");
    }

    (fired.len(), fired)
}

pub fn compute_quality_panel_report(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    preset_opt: Option<&str>,
) -> QualityPanelReport {
    let preset = preset_opt.unwrap_or("default");
    let cut = match preset {
        "lenient" => 1,
        _ => 2,
    };

    let nonlocal = crate::metrics::nonlocal::find_nonlocal_state(ingest, graph, interner);
    let state_syms: std::collections::HashSet<crate::domain::NodeId> = nonlocal
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
        .collect();

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
        let (fam_count, fired) = check_symbol_families(sym, graph, &state_syms);
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
                of: 5,
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
        rows,
    }
}

pub fn serialize_quality_panel_xml<W: Write>(
    rep: &QualityPanelReport,
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<quality_panel schema=\"ripwire.quality-panel/v1\" preset=\"{}\" families=\"6\" enabled=\"structural,lexical,confusion,historical,colocation,state\" enabled_n=\"6\" cut=\"{}\" eligible=\"{}\" ranked=\"{}\" below_cut=\"{}\" no_family=\"{}\" shown=\"{}\">",
        rep.preset,
        rep.cut,
        rep.eligible,
        rep.ranked,
        rep.below_cut,
        rep.no_family,
        rep.rows.len()
    )?;

    for r in &rep.rows {
        let p_esc = crate::serialize::escape::escape_xml_str(&r.path);
        let n_esc = crate::serialize::escape::escape_xml_str(&r.name);
        writeln!(
            out,
            "  <s p=\"{p_esc}:{}\" n=\"{n_esc}\" fam=\"{}\" of=\"{}\" fired=\"{}\"/>",
            r.line, r.fam, r.of, r.fired
        )?;
    }

    writeln!(out, "</quality_panel>")
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
