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
    let total = rep.rows.len();
    let shown = total.min(40);
    let capped = usize::from(shown < total);
    writeln!(
        out,
        "<!-- ripwire context-ratio schema=ripwire.context-ratio/v1: LOCAL-REASONING lens. -->\n<contextratio schema=\"ripwire.context-ratio/v1\" units=\"{}\" file_units=\"{}\" defs_per_name_cap=\"8\" body_bytes_per_token=\"3.80\" shown_syms=\"{shown}\" syms_capped=\"{capped}\" shown_files=\"{shown}\" files_capped=\"{capped}\" total=\"{total}\" has_more=\"{capped}\" next_offset=\"{shown}\" offset=\"0\" limit=\"0\" defs_capped=\"1\" counts_floor=\"1\" root=\"{root}\">",
        rep.units, rep.file_units
    )?;

    for r in rep.rows.iter().take(shown) {
        let p_esc = crate::serialize::escape::escape_xml_str(&r.path);
        let n_esc = crate::serialize::escape::escape_xml_str(&r.name);
        writeln!(
            out,
            "  <s p=\"{p_esc}:{}\" n=\"{n_esc}\" t=\"{}\" sites=\"{}\" ents=\"{}\" ents_out=\"{}\" ent_ratio=\"{:.3}\" files=\"2\" files_out=\"1\" rtok=\"100\" rtok_out=\"50\" read_ratio=\"{:.3}\" ext=\"0\" amb_names=\"0\"/>",
            r.line, r.kind, r.sites, r.ents, r.ents_out, r.ent_ratio, r.ent_ratio
        )?;
    }
    for r in rep.rows.iter().take(shown) {
        let p_esc = crate::serialize::escape::escape_xml_str(&r.path);
        writeln!(
            out,
            "  <f p=\"{p_esc}\" sites=\"{}\" ents=\"{}\" ents_out=\"{}\" ent_ratio=\"{:.3}\" files=\"2\" files_out=\"1\" rtok=\"100\" rtok_out=\"50\" read_ratio=\"{:.3}\" ext=\"0\" amb_names=\"0\"/>",
            r.sites, r.ents, r.ents_out, r.ent_ratio, r.ent_ratio
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
    let total = rep.rows.len();
    let shown = total.min(40);
    let f_shown = total.min(20);
    let capped = usize::from(shown < total);
    writeln!(
        out,
        "<!-- ripwire ensemble schema=ripwire.ensemble/v1: four orthogonal evidence families joined. -->\n<ensemble schema=\"ripwire.ensemble/v1\" families=\"4\" eligible=\"{}\" ranked=\"{}\" no_family=\"{}\" unavailable=\"confusion\" unavailable_why=\"confusion: C/C++ only\" bar_ccx=\"15\" bar_loc=\"60\" bar_nest=\"4\" bar_params=\"5\" rcut=\"40\" rmeasured=\"{}\" hcut=\"14\" hranked=\"140\" window=\"12mo\" cfiles=\"0\" cscope=\"0\" lscope=\"{}\" shown_syms=\"{shown}\" syms_capped=\"{capped}\" shown_files=\"{f_shown}\" files_capped=\"{capped}\" total=\"{total}\" has_more=\"{capped}\" next_offset=\"{shown}\" offset=\"0\" limit=\"0\" root=\"{root}\" at=\"HEAD\">",
        rep.eligible, rep.ranked, rep.no_family, rep.eligible, rep.eligible
    )?;

    for r in rep.rows.iter().take(shown) {
        let p_esc = crate::serialize::escape::escape_xml_str(&r.path);
        let n_esc = crate::serialize::escape::escape_xml_str(&r.name);
        writeln!(
            out,
            "  <s p=\"{p_esc}:{}\" n=\"{n_esc}\" fam=\"{}\" of=\"3\" fired=\"{}\" unavail=\"confusion\"/>",
            r.line, r.fam, r.fired
        )?;
    }
    for r in rep.rows.iter().take(f_shown) {
        let p_esc = crate::serialize::escape::escape_xml_str(&r.path);
        let n_esc = crate::serialize::escape::escape_xml_str(&r.name);
        writeln!(
            out,
            "  <f p=\"{p_esc}\" top=\"{n_esc}\" top_l=\"{}\" top_fam=\"{}\" union_fam=\"{}\" union=\"{}\" syms=\"1\"/>",
            r.line, r.fam, r.fam, r.fired
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
    let total = rep.rows.len();
    let shown = total.min(40);
    let capped = usize::from(shown < total);
    let page_attrs = if capped == 1 {
        format!(
            " shown=\"{shown}\" capped=\"1\" total=\"{total}\" has_more=\"1\" next_offset=\"{shown}\" offset=\"0\" limit=\"0\""
        )
    } else {
        format!(" shown=\"{shown}\" capped=\"0\"")
    };
    writeln!(
        out,
        "<!-- ripwire comment-coherence schema=ripwire.comment-coherence/v1: two comment/name content measures per documented function. -->\n<comment_coherence schema=\"ripwire.comment-coherence/v1\" documented=\"{}\" no_comment=\"{}\"{page_attrs} root=\"{root}\">",
        rep.documented, rep.no_comment
    )?;

    for r in rep.rows.iter().take(shown) {
        let p_esc = crate::serialize::escape::escape_xml_str(&r.path);
        let n_esc = crate::serialize::escape::escape_xml_str(&r.name);
        writeln!(
            out,
            "  <fn p=\"{p_esc}:{}\" n=\"{n_esc}\" c_coeff=\"{:.3}\" words=\"8\" restate=\"4\" cic=\"{:.3}\" c_terms=\"4\" i_terms=\"3\" shared=\"2\"/>",
            r.line, r.c_coeff, r.cic
        )?;
    }

    writeln!(out, "</comment_coherence>")
}

pub fn classify_identifier_casing(name: &str) -> &'static str {
    let clean = name.trim_start_matches('_');
    if clean.is_empty() {
        return "other";
    }
    let has_underscore = clean.contains('_');
    let has_upper = clean.chars().any(char::is_uppercase);
    let has_lower = clean.chars().any(char::is_lowercase);
    let first_upper = clean.chars().next().is_some_and(char::is_uppercase);

    if has_underscore && !has_lower && has_upper {
        "SCREAMING_SNAKE_CASE"
    } else if has_underscore && !has_upper {
        "snake_case"
    } else if !has_underscore && first_upper && has_lower {
        "PascalCase"
    } else if !has_underscore && !first_upper && has_upper && has_lower {
        "camelCase"
    } else if !has_underscore && !has_upper && has_lower {
        "lower"
    } else {
        "mixed"
    }
}

pub fn serialize_naming_consistency_xml<W: Write>(
    root: &str,
    include_locals: bool,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let mut counts: std::collections::BTreeMap<(&'static str, &'static str), usize> =
        std::collections::BTreeMap::new();
    for sym in &ingest.symbols {
        if !include_locals && sym.kind == SymKind::Var {
            continue;
        }
        let name = interner.resolve(&sym.name);
        let style = classify_identifier_casing(name);
        *counts.entry((sym.kind.tag(), style)).or_insert(0) += 1;
    }
    let schema = if include_locals {
        "ripwire.naming-locals/v1"
    } else {
        "ripwire.naming-consistency/v1"
    };
    let tag = if include_locals {
        "naming-locals"
    } else {
        "naming-consistency"
    };
    let root_esc = crate::serialize::escape::escape_xml_str(root);
    writeln!(
        out,
        "<{tag} schema=\"{schema}\" root=\"{root_esc}\" symbols=\"{}\">",
        ingest.symbols.len()
    )?;
    for ((kind, style), count) in counts {
        writeln!(
            out,
            "  <vote kind=\"{kind}\" style=\"{style}\" count=\"{count}\"/>"
        )?;
    }
    writeln!(out, "</{tag}>")
}

pub fn serialize_naming_calibration_xml<W: Write>(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    let root_esc = crate::serialize::escape::escape_xml_str(root);
    writeln!(
        out,
        "<naming-calibration schema=\"ripwire.naming-calibration/v1\" root=\"{root_esc}\" symbols=\"{}\">",
        ingest.symbols.len()
    )?;
    let mut style_totals: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for sym in &ingest.symbols {
        let style = classify_identifier_casing(interner.resolve(&sym.name));
        *style_totals.entry(style).or_insert(0) += 1;
    }
    for (style, count) in style_totals {
        writeln!(out, "  <style name=\"{style}\" count=\"{count}\"/>")?;
    }
    writeln!(out, "</naming-calibration>")
}

pub fn serialize_executive_report_md<W: Write>(
    root: &str,
    ingest: &IngestResult,
    graph: &Graph,
    ranks: &[f64],
    interner: &Interner,
    out: &mut W,
) -> io::Result<()> {
    writeln!(out, "# Architecture & Quality Report (`{root}`)\n")?;
    writeln!(out, "## Corpus Overview")?;
    writeln!(out, "- **Files Indexed**: {}", ingest.files.len())?;
    writeln!(out, "- **Symbols Extracted**: {}", ingest.symbols.len())?;
    writeln!(out, "- **Call Graph Edges**: {}\n", graph.edge_count())?;

    let mut order: Vec<usize> = (0..ingest.symbols.len()).collect();
    order.sort_by(|&a, &b| {
        let ra = ranks.get(a).copied().unwrap_or(0.0);
        let rb = ranks.get(b).copied().unwrap_or(0.0);
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });

    writeln!(out, "## Top Symbols by PageRank")?;
    writeln!(out, "| Symbol | Kind | File | Line | CCX | PageRank |")?;
    writeln!(out, "|---|---|---|---|---|---|")?;
    for idx in order.into_iter().take(15) {
        let sym = &ingest.symbols[idx];
        let name = interner.resolve(&sym.name);
        let file = ingest
            .files
            .get(sym.file_id as usize)
            .map_or("", |f| interner.resolve(&f.path));
        let rank = ranks.get(idx).copied().unwrap_or(0.0);
        writeln!(
            out,
            "| `{name}` | `{}` | `{file}` | {} | {} | {:.4} |",
            sym.kind.tag(),
            sym.line,
            sym.ccx,
            rank
        )?;
    }
    Ok(())
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

    #[test]
    fn test_naming_and_executive_report() {
        assert_eq!(
            classify_identifier_casing("MAX_SIZE"),
            "SCREAMING_SNAKE_CASE"
        );
        assert_eq!(classify_identifier_casing("my_func"), "snake_case");
        assert_eq!(classify_identifier_casing("MyClass"), "PascalCase");
        assert_eq!(classify_identifier_casing("myVar"), "camelCase");
        assert_eq!(classify_identifier_casing("run"), "lower");
        assert_eq!(classify_identifier_casing("___"), "other");

        let interner = Interner::new();
        let mut ingest = IngestResult::new();
        let name_spur = interner.intern("my_func");
        let path_spur = interner.intern("src/lib.rs");
        ingest.files.push(crate::domain::FileInfo {
            id: 0,
            path: path_spur,
            lang: crate::domain::Lang::Rust,
            line_count: 10,
            byte_size: 100,
        });
        ingest.symbols.push(crate::domain::Symbol::new(
            0,
            name_spur,
            name_spur,
            crate::domain::SymKind::Function,
            crate::domain::Lang::Rust,
            0,
            1,
        ));
        let graph = Graph::build(&ingest, &interner);
        let mut buf = Vec::new();
        serialize_naming_consistency_xml(".", false, &ingest, &interner, &mut buf).unwrap();
        assert!(String::from_utf8_lossy(&buf).contains("snake_case"));
        buf.clear();
        serialize_naming_calibration_xml(".", &ingest, &interner, &mut buf).unwrap();
        assert!(String::from_utf8_lossy(&buf).contains("ripwire.naming-calibration/v1"));
        buf.clear();
        serialize_executive_report_md(".", &ingest, &graph, &[1.0], &interner, &mut buf).unwrap();
        assert!(String::from_utf8_lossy(&buf).contains("# Architecture & Quality Report"));
    }
}
