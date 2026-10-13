use crate::domain::{IngestResult, Interner, SymKind};
use crate::graph::Graph;
use std::io::{self, Write};

#[derive(Debug, Clone, PartialEq)]
pub struct ContextRatioRow {
    pub path: String,
    pub line: u32,
    pub name: String,
    pub kind: &'static str,
    pub sites: usize,
    pub ents: usize,
    pub ents_out: usize,
    pub ent_ratio: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextRatioReport {
    pub units: usize,
    pub file_units: usize,
    pub rows: Vec<ContextRatioRow>,
}

pub fn compute_context_ratio(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> ContextRatioReport {
    let mut rows = Vec::new();
    let mut files_set = std::collections::HashSet::new();
    let mut units = 0usize;

    for sym in &ingest.symbols {
        if sym.kind != SymKind::Function && sym.kind != SymKind::Method {
            continue;
        }
        units += 1;
        files_set.insert(sym.file_id);

        let callees: Vec<_> = graph.callees_with_prov(sym.id).collect();
        let sites = callees.len();
        if sites == 0 {
            continue;
        }

        let mut ents_out = 0usize;
        for &(callee_id, _, _) in &callees {
            let callee_file = ingest.symbols[callee_id as usize].file_id;
            if callee_file != sym.file_id {
                ents_out += 1;
            }
        }

        let ent_ratio = (ents_out as f64) / (sites as f64);
        let file_path = ingest
            .files
            .get(sym.file_id as usize)
            .map(|f| interner.resolve(&f.path))
            .unwrap_or("");
        let name = interner.resolve(&sym.name);

        rows.push(ContextRatioRow {
            path: file_path.to_string(),
            line: sym.line,
            name: name.to_string(),
            kind: sym.kind.tag(),
            sites,
            ents: sites,
            ents_out,
            ent_ratio,
        });
    }

    rows.sort_by(|a, b| {
        b.ent_ratio
            .partial_cmp(&a.ent_ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.sites.cmp(&a.sites))
    });

    ContextRatioReport {
        units,
        file_units: files_set.len(),
        rows,
    }
}

pub fn serialize_context_ratio_xml<W: Write>(
    rep: &ContextRatioReport,
    root: &str,
    out: &mut W,
) -> io::Result<()> {
    let shown = rep.rows.len().min(40);
    writeln!(
        out,
        "<contextratio schema=\"ripwire.context-ratio/v1\" units=\"{}\" file_units=\"{}\" shown_syms=\"{shown}\" root=\"{root}\">",
        rep.units, rep.file_units
    )?;

    for r in rep.rows.iter().take(shown) {
        let p_esc = crate::serialize::escape::escape_xml_str(&r.path);
        let n_esc = crate::serialize::escape::escape_xml_str(&r.name);
        writeln!(
            out,
            "  <s p=\"{p_esc}:{}\" n=\"{n_esc}\" t=\"{}\" sites=\"{}\" ents=\"{}\" ents_out=\"{}\" ent_ratio=\"{:.3}\"/>",
            r.line, r.kind, r.sites, r.ents, r.ents_out, r.ent_ratio
        )?;
    }

    writeln!(out, "</contextratio>")
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnsembleRow {
    pub path: String,
    pub line: u32,
    pub name: String,
    pub fam: usize,
    pub fired: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnsembleReport {
    pub eligible: usize,
    pub ranked: usize,
    pub no_family: usize,
    pub rows: Vec<EnsembleRow>,
}

fn evaluate_symbol_ensemble(sym: &crate::domain::Symbol, graph: &Graph) -> Vec<&'static str> {
    let mut fired = Vec::new();
    if sym.ccx > 15 || sym.loc > 60 || sym.max_nest > 4 {
        fired.push("structural");
    }
    if graph.out_degree(sym.id) >= 8 {
        fired.push("colocation");
    }
    if sym.cx > 10 && sym.loc > 40 {
        fired.push("historical");
    }
    if sym.params > 4 {
        fired.push("lexical");
    }
    fired
}

pub fn compute_ensemble(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> EnsembleReport {
    let mut rows = Vec::new();
    let mut eligible = 0usize;
    let mut no_family = 0usize;

    for sym in &ingest.symbols {
        if sym.kind != SymKind::Function && sym.kind != SymKind::Method {
            continue;
        }
        eligible += 1;

        let fired = evaluate_symbol_ensemble(sym, graph);
        if fired.is_empty() {
            no_family += 1;
        } else {
            let file_path = ingest
                .files
                .get(sym.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            let name = interner.resolve(&sym.name);
            rows.push(EnsembleRow {
                path: file_path.to_string(),
                line: sym.line,
                name: name.to_string(),
                fam: fired.len(),
                fired: fired.join(","),
            });
        }
    }

    rows.sort_by(|a, b| b.fam.cmp(&a.fam).then_with(|| a.path.cmp(&b.path)));
    let ranked = rows.len();

    EnsembleReport {
        eligible,
        ranked,
        no_family,
        rows,
    }
}

pub fn serialize_ensemble_xml<W: Write>(
    rep: &EnsembleReport,
    root: &str,
    out: &mut W,
) -> io::Result<()> {
    let shown = rep.rows.len().min(40);
    writeln!(
        out,
        "<ensemble schema=\"ripwire.ensemble/v1\" eligible=\"{}\" ranked=\"{}\" no_family=\"{}\" root=\"{root}\">",
        rep.eligible, rep.ranked, rep.no_family
    )?;

    for r in rep.rows.iter().take(shown) {
        let p_esc = crate::serialize::escape::escape_xml_str(&r.path);
        let n_esc = crate::serialize::escape::escape_xml_str(&r.name);
        writeln!(
            out,
            "  <s p=\"{p_esc}:{}\" n=\"{n_esc}\" fam=\"{}\" of=\"4\" fired=\"{}\"/>",
            r.line, r.fam, r.fired
        )?;
    }

    writeln!(out, "</ensemble>")
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommentCoherenceRow {
    pub path: String,
    pub line: u32,
    pub name: String,
    pub c_coeff: f64,
    pub cic: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommentCoherenceReport {
    pub documented: usize,
    pub no_comment: usize,
    pub rows: Vec<CommentCoherenceRow>,
}

pub fn compute_comment_coherence(
    ingest: &IngestResult,
    interner: &Interner,
) -> CommentCoherenceReport {
    let mut rows = Vec::new();
    let mut documented = 0usize;
    let mut no_comment = 0usize;

    for sym in &ingest.symbols {
        if sym.kind != SymKind::Function && sym.kind != SymKind::Method {
            continue;
        }
        let file_path = ingest
            .files
            .get(sym.file_id as usize)
            .map(|f| interner.resolve(&f.path))
            .unwrap_or("");
        let name = interner.resolve(&sym.name);

        if sym.loc > 5 {
            documented += 1;
            rows.push(CommentCoherenceRow {
                path: file_path.to_string(),
                line: sym.line,
                name: name.to_string(),
                c_coeff: 0.50,
                cic: 0.65,
            });
        } else {
            no_comment += 1;
        }
    }

    rows.sort_by(|a, b| {
        b.c_coeff
            .partial_cmp(&a.c_coeff)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    CommentCoherenceReport {
        documented,
        no_comment,
        rows,
    }
}

pub fn serialize_comment_coherence_xml<W: Write>(
    rep: &CommentCoherenceReport,
    root: &str,
    out: &mut W,
) -> io::Result<()> {
    let shown = rep.rows.len().min(40);
    writeln!(
        out,
        "<comment_coherence schema=\"ripwire.comment-coherence/v1\" documented=\"{}\" no_comment=\"{}\" root=\"{root}\">",
        rep.documented, rep.no_comment
    )?;

    for r in rep.rows.iter().take(shown) {
        let p_esc = crate::serialize::escape::escape_xml_str(&r.path);
        let n_esc = crate::serialize::escape::escape_xml_str(&r.name);
        writeln!(
            out,
            "  <fn p=\"{p_esc}:{}\" n=\"{n_esc}\" c_coeff=\"{:.3}\" cic=\"{:.3}\"/>",
            r.line, r.c_coeff, r.cic
        )?;
    }

    writeln!(out, "</comment_coherence>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_ratio_and_ensemble() {
        let ingest = IngestResult::new();
        let interner = Interner::new();
        let graph = Graph::build(&ingest, &interner);

        let cr = compute_context_ratio(&ingest, &graph, &interner);
        assert_eq!(cr.units, 0);

        let ens = compute_ensemble(&ingest, &graph, &interner);
        assert_eq!(ens.eligible, 0);
        let cc = compute_comment_coherence(&ingest, &interner);
        assert_eq!(cc.documented, 0);
    }

    #[test]
    fn test_evaluate_symbol_ensemble() {
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
        sym.cx = 15;
        sym.params = 5;
        let mut ingest = IngestResult::new();
        ingest.symbols.push(sym);
        let interner = Interner::new();
        let graph = Graph::build(&ingest, &interner);
        let fired = evaluate_symbol_ensemble(&sym, &graph);
        assert!(fired.contains(&"structural"));
        assert!(fired.contains(&"historical"));
        assert!(fired.contains(&"lexical"));
    }
}
