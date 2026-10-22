use crate::domain::{IngestResult, Interner, SymKind};
use crate::graph::Graph;
use crate::query::safety::SafetyContext;
use crate::serialize::{PageRequest, PageWindow, XmlEscapeExt, row_bytes, text_bytes};
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
    pub files: usize,
    pub files_out: usize,
    pub rtok: usize,
    pub rtok_out: usize,
    pub read_ratio: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextRatioReport {
    pub units: usize,
    pub file_units: usize,
    pub rows: Vec<ContextRatioRow>,
    pub page: PageRequest,
}

fn estimate_symbol_bytes(sym: &crate::domain::Symbol) -> usize {
    (sym.end_byte.saturating_sub(sym.sig_start_byte) as usize)
        .max((sym.loc as usize).saturating_mul(38))
        .max(4)
}

fn build_context_ratio_row(
    sym: &crate::domain::Symbol,
    callees: &[crate::graph::CalleeEdge],
    ingest: &IngestResult,
    interner: &Interner,
) -> ContextRatioRow {
    let sites = callees.len();
    let mut ents_out = 0usize;
    let mut touched_files = std::collections::HashSet::new();
    touched_files.insert(sym.file_id);
    let mut ext_files = std::collections::HashSet::new();
    let mut total_bytes = estimate_symbol_bytes(sym);
    let mut out_bytes = 0usize;

    for edge in callees {
        let callee = &ingest[edge.to];
        let c_bytes = estimate_symbol_bytes(callee);
        total_bytes += c_bytes;
        touched_files.insert(callee.file_id);
        if callee.file_id != sym.file_id {
            ents_out += 1;
            ext_files.insert(callee.file_id);
            out_bytes += c_bytes;
        }
    }

    let ent_ratio = (ents_out as f64) / (sites as f64);
    let rtok = ((total_bytes as f64) / 3.8).round().max(1.0) as usize;
    let rtok_out = ((out_bytes as f64) / 3.8).round() as usize;
    let read_ratio = (rtok_out as f64) / (rtok as f64);
    let file_path = ingest.sym_file_path(sym, interner);
    let name = &interner[sym.name];

    ContextRatioRow {
        path: file_path.to_string(),
        line: sym.line,
        name: name.to_string(),
        kind: sym.kind.tag(),
        sites,
        ents: sites,
        ents_out,
        ent_ratio,
        files: touched_files.len(),
        files_out: ext_files.len(),
        rtok,
        rtok_out,
        read_ratio,
    }
}

pub fn compute_context_ratio(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    page: PageRequest,
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
        let callees: Vec<_> = graph.callee_edges(sym.id).collect();
        if !callees.is_empty() {
            rows.push(build_context_ratio_row(sym, &callees, ingest, interner));
        }
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
        page,
    }
}

impl ContextRatioReport {
    fn window(&self) -> PageWindow {
        self.page.window_fit(self.rows.len(), |i| {
            let r = &self.rows[i];
            row_bytes(|w| write_ratio_sym_row(r, w)) + row_bytes(|w| write_ratio_file_row(r, w))
        })
    }
}

fn write_ratio_sym_row<W: Write + ?Sized>(r: &ContextRatioRow, w: &mut W) -> io::Result<()> {
    writeln!(
        w,
        "  <s p=\"{}:{}\" n=\"{}\" t=\"{}\" sites=\"{}\" ents=\"{}\" ents_out=\"{}\" ent_ratio=\"{:.3}\" files=\"{}\" files_out=\"{}\" rtok=\"{}\" rtok_out=\"{}\" read_ratio=\"{:.3}\" ext=\"0\" amb_names=\"0\"/>",
        r.path.xml_escaped(),
        r.line,
        r.name.xml_escaped(),
        r.kind,
        r.sites,
        r.ents,
        r.ents_out,
        r.ent_ratio,
        r.files,
        r.files_out,
        r.rtok,
        r.rtok_out,
        r.read_ratio
    )
}

fn write_ratio_file_row<W: Write + ?Sized>(r: &ContextRatioRow, w: &mut W) -> io::Result<()> {
    writeln!(
        w,
        "  <f p=\"{}\" sites=\"{}\" ents=\"{}\" ents_out=\"{}\" ent_ratio=\"{:.3}\" files=\"{}\" files_out=\"{}\" rtok=\"{}\" rtok_out=\"{}\" read_ratio=\"{:.3}\" ext=\"0\" amb_names=\"0\"/>",
        r.path.xml_escaped(),
        r.sites,
        r.ents,
        r.ents_out,
        r.ent_ratio,
        r.files,
        r.files_out,
        r.rtok,
        r.rtok_out,
        r.read_ratio
    )
}

impl crate::serialize::XmlReport for ContextRatioReport {
    crate::xml_report_meta!(
        "contextratio",
        "ripwire-rs.context-ratio/v1",
        root: true,
        comment: "ripwire-rs context-ratio schema=ripwire-rs.context-ratio/v1: LOCAL-REASONING lens."
    );
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let pw = self.window();
        write!(
            w,
            " units=\"{}\" file_units=\"{}\" defs_per_name_cap=\"8\" body_bytes_per_token=\"3.80\" shown_syms=\"{}\" syms_capped=\"{}\" shown_files=\"{}\" files_capped=\"{}\" total=\"{}\" has_more=\"{}\" next_offset=\"{}\" offset=\"{}\" limit=\"{}\" defs_capped=\"1\" counts_floor=\"1\"",
            self.units,
            self.file_units,
            pw.shown,
            usize::from(pw.capped),
            pw.shown,
            usize::from(pw.capped),
            pw.total,
            usize::from(pw.has_more),
            pw.next_offset,
            pw.offset,
            pw.limit
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let pw = self.window();
        let page_rows = &self.rows[pw.begin..pw.end];
        for r in page_rows {
            write_ratio_sym_row(r, w)?;
        }
        for r in page_rows {
            write_ratio_file_row(r, w)?;
        }
        Ok(())
    }
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
    pub cfiles: usize,
    pub cscope: usize,
    pub hranked: usize,
    pub rows: Vec<EnsembleRow>,
    pub page: PageRequest,
}

fn evaluate_symbol_ensemble(sym: &crate::domain::Symbol, graph: &Graph) -> Vec<&'static str> {
    let ctx = crate::metrics::quality_panel::QualityFamilyContext {
        graph,
        state_syms: None,
    };
    crate::metrics::quality_panel::evaluate_quality_families(
        sym,
        &ctx,
        crate::metrics::quality_panel::ENSEMBLE_FAMILIES,
    )
}

pub fn compute_ensemble(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    page: PageRequest,
) -> EnsembleReport {
    let cfiles = ingest
        .files
        .iter()
        .filter(|f| f.lang.semantics().supports_c_layout())
        .count();
    let mut cscope = 0usize;
    let mut hranked = 0usize;
    let mut rows = Vec::new();
    let mut eligible = 0usize;
    let mut no_family = 0usize;

    for sym in &ingest.symbols {
        if sym.kind != SymKind::Function && sym.kind != SymKind::Method {
            continue;
        }
        eligible += 1;
        cscope += usize::from(sym.lang.semantics().supports_c_layout());
        let fired = evaluate_symbol_ensemble(sym, graph);
        hranked += usize::from(fired.contains(&"historical"));
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
        cfiles,
        cscope,
        hranked,
        rows,
        page,
    }
}

impl EnsembleReport {
    fn window(&self) -> PageWindow {
        self.page.window_fit(self.rows.len(), |i| {
            let r = &self.rows[i];
            row_bytes(|w| write_ensemble_sym_row(r, w))
                + row_bytes(|w| write_ensemble_file_row(r, w))
        })
    }
}

fn write_ensemble_sym_row<W: Write + ?Sized>(r: &EnsembleRow, w: &mut W) -> io::Result<()> {
    writeln!(
        w,
        "  <s p=\"{}:{}\" n=\"{}\" fam=\"{}\" of=\"3\" fired=\"{}\" unavail=\"confusion\"/>",
        r.path.xml_escaped(),
        r.line,
        r.name.xml_escaped(),
        r.fam,
        r.fired
    )
}

fn write_ensemble_file_row<W: Write + ?Sized>(r: &EnsembleRow, w: &mut W) -> io::Result<()> {
    writeln!(
        w,
        "  <f p=\"{}\" top=\"{}\" top_l=\"{}\" top_fam=\"{}\" union_fam=\"{}\" union=\"{}\" syms=\"1\"/>",
        r.path.xml_escaped(),
        r.name.xml_escaped(),
        r.line,
        r.fam,
        r.fam,
        r.fired
    )
}

impl crate::serialize::XmlReport for EnsembleReport {
    crate::xml_report_meta!(
        "ensemble",
        "ripwire-rs.ensemble/v1",
        root: true,
        comment: "ripwire-rs ensemble schema=ripwire-rs.ensemble/v1: four orthogonal evidence families joined."
    );
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let pw = self.window();
        let f_shown = pw.shown;
        write!(
            w,
            " families=\"4\" eligible=\"{}\" ranked=\"{}\" no_family=\"{}\" unavailable=\"confusion\" unavailable_why=\"confusion: C/C++ only\" bar_ccx=\"15\" bar_loc=\"60\" bar_nest=\"4\" bar_params=\"5\" rcut=\"40\" rmeasured=\"{}\" hcut=\"14\" hranked=\"{}\" window=\"12mo\" cfiles=\"{}\" cscope=\"{}\" lscope=\"{}\" shown_syms=\"{}\" syms_capped=\"{}\" shown_files=\"{f_shown}\" files_capped=\"{}\" total=\"{}\" has_more=\"{}\" next_offset=\"{}\" offset=\"{}\" limit=\"{}\" at=\"HEAD\"",
            self.eligible,
            self.ranked,
            self.no_family,
            self.eligible,
            self.hranked,
            self.cfiles,
            self.cscope,
            self.eligible,
            pw.shown,
            usize::from(pw.capped),
            usize::from(pw.capped),
            pw.total,
            usize::from(pw.has_more),
            pw.next_offset,
            pw.offset,
            pw.limit
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let pw = self.window();
        let page_rows = &self.rows[pw.begin..pw.end];
        for r in page_rows {
            write_ensemble_sym_row(r, w)?;
        }
        for r in page_rows {
            write_ensemble_file_row(r, w)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommentCoherenceRow {
    pub path: String,
    pub line: u32,
    pub name: String,
    pub words: usize,
    pub restate: usize,
    pub c_terms: usize,
    pub i_terms: usize,
    pub shared: usize,
    pub c_coeff: f64,
    pub cic: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommentCoherenceReport {
    pub documented: usize,
    pub no_comment: usize,
    pub rows: Vec<CommentCoherenceRow>,
    pub page: PageRequest,
}

fn split_identifier_subtokens(ident: &str) -> std::collections::BTreeSet<String> {
    let mut tokens: std::collections::BTreeSet<String> = super::eval::split_camel_or_snake(ident)
        .into_iter()
        .collect();
    if tokens.is_empty() && !ident.is_empty() {
        tokens.insert(ident.to_ascii_lowercase());
    }
    tokens
}

fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with('#')
}

fn is_doc_boundary_line(trimmed: &str) -> bool {
    trimmed.is_empty() || trimmed.starts_with("#[") || trimmed.starts_with('@')
}

fn extract_preceding_doc_words(bytes: &[u8], sig_start: u32) -> Vec<String> {
    let end = (sig_start as usize).min(bytes.len());
    let Ok(prefix) = std::str::from_utf8(&bytes[..end]) else {
        return Vec::new();
    };
    let mut doc_lines = Vec::new();
    for line in prefix.lines().rev() {
        let trimmed = line.trim();
        if doc_lines.is_empty() && is_doc_boundary_line(trimmed) {
            continue;
        }
        if !is_comment_line(trimmed) {
            break;
        }
        doc_lines.push(trimmed);
    }
    doc_lines.reverse();
    doc_lines
        .into_iter()
        .flat_map(|l| l.split(|c: char| !c.is_ascii_alphanumeric()))
        .filter(|w| w.len() >= 2)
        .map(str::to_ascii_lowercase)
        .collect()
}

fn measure_comment_coherence_row(
    sym: &crate::domain::Symbol,
    file_path: &str,
    name: &str,
    file_bytes: Option<&[u8]>,
) -> Option<CommentCoherenceRow> {
    let ident_terms = split_identifier_subtokens(name);
    let comment_words = match file_bytes {
        Some(bytes) => extract_preceding_doc_words(bytes, sym.sig_start_byte),
        None if sym.loc > 5 => ident_terms.iter().cloned().collect(),
        None => Vec::new(),
    };
    let words = comment_words.len();
    if words < 2 {
        return None;
    }
    let restate = comment_words
        .iter()
        .filter(|w| {
            ident_terms
                .iter()
                .any(|it| w.contains(it) || it.contains(*w))
        })
        .count();
    let c_set: std::collections::BTreeSet<String> = comment_words.into_iter().collect();
    let c_terms = c_set.len();
    let i_terms = ident_terms.len();
    let shared = c_set
        .iter()
        .filter(|w| {
            ident_terms
                .iter()
                .any(|it| w.contains(it) || it.contains(*w))
        })
        .count();
    let union = (c_terms + i_terms).saturating_sub(shared).max(1);
    let c_coeff = (restate as f64) / (words.max(1) as f64);
    let cic = (shared as f64) / (union as f64);
    Some(CommentCoherenceRow {
        path: file_path.to_string(),
        line: sym.line,
        name: name.to_string(),
        words,
        restate,
        c_terms,
        i_terms,
        shared,
        c_coeff,
        cic,
    })
}

pub fn compute_comment_coherence(
    ingest: &IngestResult,
    interner: &Interner,
    page: PageRequest,
) -> CommentCoherenceReport {
    compute_comment_coherence_in_root(".", ingest, interner, page)
}

pub fn compute_comment_coherence_in_root(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    page: PageRequest,
) -> CommentCoherenceReport {
    let sources = crate::query::FileSources {
        root: std::path::Path::new(root),
    };
    compute_comment_coherence_with_sources(&sources, ingest, interner, page)
}

pub fn compute_comment_coherence_with_sources<S: crate::query::SourceReader + ?Sized>(
    sources: &S,
    ingest: &IngestResult,
    interner: &Interner,
    page: PageRequest,
) -> CommentCoherenceReport {
    let mut rows = Vec::new();
    let mut documented = 0usize;
    let mut no_comment = 0usize;
    let mut file_cache: std::collections::HashMap<u32, Option<Vec<u8>>> =
        std::collections::HashMap::new();

    use crate::domain::CorpusView;
    let view = (ingest, interner);
    for sym in &ingest.symbols {
        if (sym.kind != SymKind::Function && sym.kind != SymKind::Method)
            || view.is_test_symbol(sym)
        {
            continue;
        }
        let file_path = view.sym_path(sym);
        let name = view.sym_name(sym);
        let file_bytes = file_cache
            .entry(sym.file_id)
            .or_insert_with(|| sources.read(file_path).ok().map(|c| c.into_owned()))
            .as_deref();
        if let Some(row) = measure_comment_coherence_row(sym, file_path, name, file_bytes) {
            documented += 1;
            rows.push(row);
        } else {
            no_comment += 1;
        }
    }

    rows.sort_by(|a, b| {
        b.c_coeff
            .partial_cmp(&a.c_coeff)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
    });

    CommentCoherenceReport {
        documented,
        no_comment,
        rows,
        page,
    }
}

impl CommentCoherenceReport {
    fn window(&self) -> PageWindow {
        self.page.window_fit(self.rows.len(), |i| {
            row_bytes(|w| write_comment_coherence_row(&self.rows[i], w))
        })
    }
}

fn write_comment_coherence_row<W: Write + ?Sized>(
    r: &CommentCoherenceRow,
    w: &mut W,
) -> io::Result<()> {
    writeln!(
        w,
        "  <fn p=\"{}:{}\" n=\"{}\" c_coeff=\"{:.3}\" words=\"{}\" restate=\"{}\" cic=\"{:.3}\" c_terms=\"{}\" i_terms=\"{}\" shared=\"{}\"/>",
        r.path.xml_escaped(),
        r.line,
        r.name.xml_escaped(),
        r.c_coeff,
        r.words,
        r.restate,
        r.cic,
        r.c_terms,
        r.i_terms,
        r.shared
    )
}

impl crate::serialize::XmlReport for CommentCoherenceReport {
    crate::xml_report_meta!(
        "comment_coherence",
        "ripwire-rs.comment-coherence/v1",
        root: true,
        comment: "ripwire-rs comment-coherence schema=ripwire-rs.comment-coherence/v1: two comment/name content measures per documented function."
    );
    fn page_window(&self) -> Option<crate::serialize::PageWindow> {
        Some(self.window())
    }
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        write!(
            w,
            " documented=\"{}\" no_comment=\"{}\"",
            self.documented, self.no_comment
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let pw = self.window();
        for r in &self.rows[pw.begin..pw.end] {
            write_comment_coherence_row(r, w)?;
        }
        Ok(())
    }
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

/// Root and scope for the naming-consistency serializer.
pub struct NamingScope<'a> {
    pub root: &'a str,
    pub include_locals: bool,
}

fn write_vote_row<W: Write + ?Sized>(
    w: &mut W,
    kind: &str,
    style: &str,
    count: usize,
) -> io::Result<()> {
    writeln!(
        w,
        "  <vote kind=\"{kind}\" style=\"{style}\" count=\"{count}\"/>"
    )
}

fn write_style_row<W: Write + ?Sized>(w: &mut W, name: &str, count: usize) -> io::Result<()> {
    writeln!(w, "  <style name=\"{name}\" count=\"{count}\"/>")
}

pub fn serialize_naming_consistency_xml<W: Write>(
    scope: NamingScope<'_>,
    ingest: &IngestResult,
    interner: &Interner,
    page: PageRequest,
    out: &mut W,
) -> io::Result<()> {
    let NamingScope {
        root,
        include_locals,
    } = scope;
    let mut counts: std::collections::BTreeMap<(&'static str, &'static str), usize> =
        std::collections::BTreeMap::new();
    for sym in &ingest.symbols {
        if sym.kind == SymKind::ModuleScope || (!include_locals && sym.kind == SymKind::Var) {
            continue;
        }
        let name = interner.resolve(&sym.name);
        let style = classify_identifier_casing(name);
        *counts.entry((sym.kind.tag(), style)).or_insert(0) += 1;
    }
    let schema = if include_locals {
        "ripwire-rs.naming-locals/v1"
    } else {
        "ripwire-rs.naming-consistency/v1"
    };
    let tag = if include_locals {
        "naming-locals"
    } else {
        "naming-consistency"
    };
    writeln!(
        out,
        "<{tag} schema=\"{schema}\" root=\"{}\" symbols=\"{}\">",
        root.xml_escaped(),
        ingest.symbol_count()
    )?;
    let votes: Vec<_> = counts.into_iter().collect();
    let pw = page.window_fit(votes.len(), |i| {
        let ((kind, style), count) = &votes[i];
        row_bytes(|w| write_vote_row(w, kind, style, *count))
    });
    for ((kind, style), count) in &votes[pw.begin..pw.end] {
        write_vote_row(out, kind, style, *count)?;
    }
    writeln!(out, "</{tag}>")
}

pub fn serialize_naming_calibration_xml<W: Write>(
    root: &str,
    ingest: &IngestResult,
    interner: &Interner,
    page: PageRequest,
    out: &mut W,
) -> io::Result<()> {
    writeln!(
        out,
        "<naming-calibration schema=\"ripwire-rs.naming-calibration/v1\" root=\"{}\" symbols=\"{}\">",
        root.xml_escaped(),
        ingest.symbol_count()
    )?;
    let mut style_totals: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for sym in &ingest.symbols {
        let style = classify_identifier_casing(interner.resolve(&sym.name));
        *style_totals.entry(style).or_insert(0) += 1;
    }
    let styles: Vec<_> = style_totals.into_iter().collect();
    let pw = page.window_fit(styles.len(), |i| {
        let (style, count) = styles[i];
        row_bytes(|w| write_style_row(w, style, count))
    });
    for (style, count) in &styles[pw.begin..pw.end] {
        write_style_row(out, style, *count)?;
    }
    writeln!(out, "</naming-calibration>")
}

fn extract_module_intents(members: &[(&'static str, &str, &str, u32)]) -> String {
    const VERBS: &[&str] = &[
        "create",
        "make",
        "get",
        "parse",
        "build",
        "run",
        "resolve",
        "serialize",
        "check",
        "execute",
        "compute",
        "find",
        "load",
        "write",
    ];
    let mut counts: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    for &(_, name, _, _) in members {
        let lower = name.to_ascii_lowercase();
        for &v in VERBS {
            if lower.starts_with(v) {
                *counts.entry(v).or_default() += 1;
                break;
            }
        }
    }
    let mut ranked: Vec<(&'static str, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    if ranked.is_empty() {
        String::new()
    } else {
        let tags: Vec<&str> = ranked.into_iter().take(3).map(|(v, _)| v).collect();
        format!(" `[{}]`", tags.join(", "))
    }
}

fn non_empty_or<'a>(s: &'a str, fallback: &'a str) -> &'a str {
    if s.is_empty() { fallback } else { s }
}

fn write_report_modules_and_bridges<W: Write>(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    page: PageRequest,
    out: &mut W,
) -> io::Result<()> {
    let (comms, _, modules) =
        crate::metrics::architecture::build_ranked_communities(None, ingest, graph, interner);
    let pw = page.window_fit(modules.len(), |i| {
        text_bytes(&[modules[i].label, modules[i].dir])
    });
    writeln!(
        out,
        "## Modules (call-graph clusters; showing {} of {})",
        pw.shown,
        modules.len()
    )?;
    if modules.is_empty() {
        writeln!(out, "- (none)\n")?;
    } else {
        for m in &modules[pw.begin..pw.end] {
            let intents = extract_module_intents(&m.members);
            writeln!(
                out,
                "- **{}** (`{}`) — {} symbols{intents}",
                m.label, m.dir, m.size
            )?;
        }
        writeln!(out)?;
    }

    let bridges: Vec<_> =
        crate::metrics::architecture::collect_community_bridges(&comms, &modules, ingest, graph)
            .into_iter()
            .map(|(_, _, la, lb, w)| (non_empty_or(la, "module"), non_empty_or(lb, "module"), w))
            .collect();
    let pw = page.window_fit(bridges.len(), |i| text_bytes(&[bridges[i].0, bridges[i].1]));
    writeln!(
        out,
        "## Cross-module bridges (showing {} of {})",
        pw.shown,
        bridges.len()
    )?;
    if bridges.is_empty() {
        writeln!(out, "- (none)\n")?;
    } else {
        for (la, lb, w) in &bridges[pw.begin..pw.end] {
            writeln!(out, "- `{la}` ↔ `{lb}` ({w} edges)")?;
        }
        writeln!(out)?;
    }
    Ok(())
}

fn write_report_deps_sections<W: Write>(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    page: PageRequest,
    out: &mut W,
) -> io::Result<()> {
    let deps = crate::metrics::architecture::compute_deps_report(ingest, graph, interner);
    let mut god: Vec<&crate::serialize::DepsFileRow<'_>> =
        deps.rows.iter().filter(|r| r.afferent > 0).collect();
    god.sort_by(|a, b| b.afferent.cmp(&a.afferent).then_with(|| a.path.cmp(b.path)));
    let pw = page.window_fit(god.len(), |i| text_bytes(&[god[i].path]));
    writeln!(
        out,
        "## God files (most depended-on; showing {} of {})",
        pw.shown,
        god.len()
    )?;
    if god.is_empty() {
        writeln!(out, "- (no include/import edges captured)\n")?;
    } else {
        for g in &god[pw.begin..pw.end] {
            writeln!(out, "- `{}` — {} dependents", g.path, g.afferent)?;
        }
        writeln!(out)?;
    }

    let pw = page.window_fit(deps.cycles.len(), |i| text_bytes(&deps.cycles[i]));
    writeln!(
        out,
        "## Dependency cycles (showing {} of {})",
        pw.shown,
        deps.cycles.len()
    )?;
    if deps.cycles.is_empty() {
        writeln!(out, "- none (acyclic)\n")?;
    } else {
        for cyc in &deps.cycles[pw.begin..pw.end] {
            let formatted: Vec<String> = cyc.iter().map(|p| format!("`{p}`")).collect();
            writeln!(out, "- {}", formatted.join(" ↔ "))?;
        }
        writeln!(out)?;
    }
    Ok(())
}

pub fn serialize_executive_report_md<W: Write>(
    facts: &SafetyContext<'_>,
    ranks: &[f64],
    page: PageRequest,
    out: &mut W,
) -> io::Result<()> {
    let SafetyContext {
        root,
        ingest,
        graph,
        interner,
    } = *facts;
    writeln!(out, "# Architecture & Quality Report (`{root}`)\n")?;
    writeln!(out, "## Corpus Overview")?;
    writeln!(out, "- **Files Indexed**: {}", ingest.files.len())?;
    writeln!(out, "- **Symbols Extracted**: {}", ingest.symbol_count())?;
    writeln!(out, "- **Call Graph Edges**: {}\n", graph.edge_count())?;

    write_report_modules_and_bridges(ingest, graph, interner, page, out)?;
    write_report_deps_sections(ingest, graph, interner, page, out)?;

    use crate::domain::CorpusView;
    let view = (ingest, interner);
    let mut order: Vec<usize> = (0..ingest.symbols.len())
        .filter(|&i| view.is_production_symbol(&ingest.symbols[i]))
        .collect();
    if order.is_empty() {
        order = (0..ingest.symbols.len()).collect();
    }
    order.sort_by(|&a, &b| {
        let ra = ranks.get(a).copied().unwrap_or(0.0);
        let rb = ranks.get(b).copied().unwrap_or(0.0);
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });
    let pw = page.window_fit(order.len(), |i| {
        let sym = &ingest.symbols[order[i]];
        text_bytes(&[view.sym_name(sym), sym.kind.tag(), view.sym_path(sym)])
    });
    writeln!(out, "## Top Symbols by PageRank")?;
    writeln!(out, "| Symbol | Kind | File | Line | CCX | PageRank |")?;
    writeln!(out, "|---|---|---|---|---|---|")?;
    for &idx in &order[pw.begin..pw.end] {
        let sym = &ingest.symbols[idx];
        let name = view.sym_name(sym);
        let file = view.sym_path(sym);
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
