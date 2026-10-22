use crate::domain::{IngestResult, Interner, NodeId, Symbol};
use crate::git::history::{
    CommitInfo, DEFAULT_CHURN_MONTHS, FileChurn, MERGE_BOMB_MAX_FILES, civil_from_days,
    compute_file_churn, compute_git_at_stamp, resolve_cutoff_timestamp,
    walk_repository_history_scoped,
};
use crate::graph::Graph;
use crate::metrics::lint_naming::{is_short_name, is_wordy_name, words};
use crate::serialize::{PageRequest, PageWindow, XmlEscapeExt, row_bytes};
use std::collections::HashMap;
use std::io::{self, Write};

/// Historical ceiling: a file with this many commits in the window always fires, so a busy repository
/// keeps the fixed cut it always had.
pub const HISTORY_CUT_CEILING: u32 = 14;
/// Historical floor: no file fires on fewer window commits, so a quiet repository's hottest files surface
/// while a file touched once or twice never does.
pub const HISTORY_CUT_FLOOR: u32 = 3;
const SECS_PER_DAY: i64 = 86_400;

#[derive(Debug, Clone, PartialEq)]
pub struct FamilyEvidence {
    pub family: &'static str,
    /// Pre-rendered measured values, each attribute prefixed with a space.
    pub attrs: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QualityPanelRow {
    pub path: String,
    pub line: u32,
    pub name: String,
    pub fam: usize,
    pub of: usize,
    pub fired: String,
    pub evidence: Vec<FamilyEvidence>,
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
    pub enabled: Vec<&'static str>,
    pub unavailable: Vec<(&'static str, String)>,
    pub history: Result<PanelHistory, String>,
    pub rows: Vec<QualityPanelRow>,
    pub page: PageRequest,
}

/// Per-file git churn backing the historical family, over the window that was walked.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelHistory {
    /// Short HEAD stamp (`sha[+dirty]`) the churn was read at.
    pub at: String,
    /// Window start (epoch seconds); commits older than this are excluded.
    pub since: i64,
    /// Commits in the window a file needs before the historical family fires.
    pub cut: u32,
    /// What set `cut`: `p90` (90th percentile of changed files), `floor` or `ceiling`.
    pub cut_basis: &'static str,
    /// Indexed files with at least one commit in the window: the population `cut` is read from.
    pub measured: usize,
    /// Of `measured`, the files at or above `cut`.
    pub crossed: usize,
    /// Churn indexed by ingest file id.
    churn: Vec<Option<FileChurn>>,
}

impl PanelHistory {
    /// Builds churn from commits, keeping only those at or after `since`.
    #[must_use]
    pub fn from_commits(
        ingest: &IngestResult,
        interner: &Interner,
        commits: &[CommitInfo],
        at: String,
        since: i64,
    ) -> Self {
        let window: Vec<&CommitInfo> = commits.iter().filter(|c| c.timestamp >= since).collect();
        let head = window.first().map_or(0, |c| c.timestamp);
        let by_path = compute_file_churn(window, head, MERGE_BOMB_MAX_FILES);
        let churn: Vec<Option<FileChurn>> = ingest
            .files
            .iter()
            .map(|f| by_path.get(interner.resolve(&f.path)).cloned())
            .collect();
        let (cut, cut_basis, measured, crossed) = history_cut(&churn);
        Self {
            at,
            since,
            cut,
            cut_basis,
            measured,
            crossed,
            churn,
        }
    }

    /// Walks the default churn window ending at HEAD of `root`.
    /// `Err` names why git history cannot back the historical family.
    pub fn load(root: &str, ingest: &IngestResult, interner: &Interner) -> Result<Self, String> {
        let repo = crate::git::repo::discover(root).ok_or("not a git repository")?;
        if repo.git_dir().join("shallow").exists() {
            return Err("shallow clone: history is truncated".to_string());
        }
        let since = resolve_cutoff_timestamp(None, DEFAULT_CHURN_MONTHS).unwrap_or(0);
        let commits =
            walk_repository_history_scoped(root, Some(since)).map_err(|e| e.to_string())?;
        let at = compute_git_at_stamp(root).ok_or("HEAD has no commit stamp")?;
        Ok(Self::from_commits(ingest, interner, &commits, at, since))
    }
}

/// The historical cut: the 90th percentile of window commits across changed files, clamped to
/// `[HISTORY_CUT_FLOOR, HISTORY_CUT_CEILING]`. Returns `(cut, basis, changed files, files at or above cut)`.
fn history_cut(churn: &[Option<FileChurn>]) -> (u32, &'static str, usize, usize) {
    let mut counts: Vec<u32> = churn.iter().flatten().map(|c| c.commits).collect();
    counts.sort_unstable();
    let n = counts.len();
    if n == 0 {
        return (HISTORY_CUT_FLOOR, "floor", 0, 0);
    }
    let p90 = counts[(9 * n).div_ceil(10) - 1];
    let basis = if p90 > HISTORY_CUT_CEILING {
        "ceiling"
    } else if p90 < HISTORY_CUT_FLOOR {
        "floor"
    } else {
        "p90"
    };
    let cut = p90.clamp(HISTORY_CUT_FLOOR, HISTORY_CUT_CEILING);
    (cut, basis, n, counts.iter().filter(|&&c| c >= cut).count())
}

pub(crate) fn iso_date(epoch: i64) -> String {
    let (y, m, d) = civil_from_days(epoch.div_euclid(SECS_PER_DAY));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Historical root attributes. `hstatus` says whether the family is `ranked` (rows fired), `measured-empty`
/// (churn read, no function ranked) or `unavailable` (no history; `unavailable=` names why). `window`, `at`,
/// `hcut`, `hbasis` (`p90`, `floor` or `ceiling`), `hfiles` (changed files measured) and `hcrossed` (of
/// those, files at or above `hcut`) follow when history was read.
pub(crate) fn history_root_attrs(history: &Result<PanelHistory, String>, hranked: usize) -> String {
    match history {
        Err(_) => " hstatus=\"unavailable\"".to_string(),
        Ok(h) => format!(
            " window=\"12mo@{}\" at=\"{}\" hcut=\"{}\" hbasis=\"{}\" hfiles=\"{}\" hcrossed=\"{}\" hstatus=\"{}\"",
            iso_date(h.since),
            h.at.as_str().xml_escaped(),
            h.cut,
            h.cut_basis,
            h.measured,
            h.crossed,
            if hranked == 0 {
                "measured-empty"
            } else {
                "ranked"
            }
        ),
    }
}

pub struct QualityFamilyContext<'a> {
    pub graph: &'a Graph,
    pub interner: &'a Interner,
    pub state_syms: Option<&'a HashMap<NodeId, StateAccess>>,
    pub history: Option<&'a PanelHistory>,
}

impl QualityFamilyContext<'_> {
    fn file_churn(&self, sym: &Symbol) -> Option<&FileChurn> {
        self.history
            .and_then(|h| h.churn.get(sym.file_id as usize))
            .and_then(Option::as_ref)
    }
}

/// Reads and writes of nonlocal state cells made by one function.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StateAccess {
    pub reads: usize,
    pub writes: usize,
}

/// Trait encapsulating an orthogonal symbol quality evidence family.
pub trait QualityEvidenceFamily: Sync {
    fn name(&self) -> &'static str;
    /// Whether the family is meaningful for this symbol's language.
    fn applies(&self, sym: &Symbol) -> bool;
    fn fires(&self, sym: &Symbol, ctx: &QualityFamilyContext<'_>) -> bool;
    fn evidence(&self, sym: &Symbol, ctx: &QualityFamilyContext<'_>) -> String;
}

pub struct QualityFamilySpec {
    pub name: &'static str,
    pub c_only: bool,
    pub predicate: fn(&Symbol, &QualityFamilyContext<'_>) -> bool,
    pub measure: fn(&Symbol, &QualityFamilyContext<'_>) -> String,
}

impl QualityEvidenceFamily for QualityFamilySpec {
    fn name(&self) -> &'static str {
        self.name
    }
    fn applies(&self, sym: &Symbol) -> bool {
        !self.c_only || sym.lang.semantics().supports_c_layout()
    }
    fn fires(&self, sym: &Symbol, ctx: &QualityFamilyContext<'_>) -> bool {
        (self.predicate)(sym, ctx)
    }
    fn evidence(&self, sym: &Symbol, ctx: &QualityFamilyContext<'_>) -> String {
        (self.measure)(sym, ctx)
    }
}

fn is_structural_base(sym: &Symbol) -> bool {
    sym.ccx > 15 || sym.loc > 60 || sym.max_nest > 4
}

fn structural_measure(sym: &Symbol, _: &QualityFamilyContext<'_>) -> String {
    let humps = nonzero_attr("humps", sym.humps);
    let deep = nonzero_attr("deep", sym.deep);
    format!(
        " ccx=\"{}\" loc=\"{}\" nest=\"{}\" params=\"{}\"{humps}{deep}",
        sym.ccx, sym.loc, sym.max_nest, sym.params
    )
}

/// ` name="value"` for an informative (non-zero) measure, else nothing: present-only serialization.
fn nonzero_attr(name: &str, value: u16) -> String {
    if value == 0 {
        String::new()
    } else {
        format!(" {name}=\"{value}\"")
    }
}

fn lexical_fires(sym: &Symbol, ctx: &QualityFamilyContext<'_>) -> bool {
    let name = ctx.interner.resolve(&sym.name);
    is_short_name(name) || is_wordy_name(name)
}

fn lexical_measure(sym: &Symbol, ctx: &QualityFamilyContext<'_>) -> String {
    let name = ctx.interner.resolve(&sym.name);
    format!(
        " name_len=\"{}\" name_words=\"{}\"",
        name.chars().count(),
        words(name).len()
    )
}

const LEXICAL_FAMILY: QualityFamilySpec = QualityFamilySpec {
    name: "lexical",
    c_only: false,
    predicate: lexical_fires,
    measure: lexical_measure,
};

fn confusion_fires(sym: &Symbol, _: &QualityFamilyContext<'_>) -> bool {
    sym.lang.semantics().supports_c_layout()
        && (sym.humps > 2 || (sym.max_nest >= 4 && sym.cx >= 12) || sym.ccx > 20)
}

fn confusion_measure(sym: &Symbol, _: &QualityFamilyContext<'_>) -> String {
    format!(
        " humps=\"{}\" nest=\"{}\" cx=\"{}\" ccx=\"{}\"",
        sym.humps, sym.max_nest, sym.cx, sym.ccx
    )
}

const CONFUSION_FAMILY: QualityFamilySpec = QualityFamilySpec {
    name: "confusion",
    c_only: true,
    predicate: confusion_fires,
    measure: confusion_measure,
};

fn historical_fires(sym: &Symbol, ctx: &QualityFamilyContext<'_>) -> bool {
    ctx.history
        .is_some_and(|h| ctx.file_churn(sym).is_some_and(|c| c.commits >= h.cut))
}

fn historical_measure(sym: &Symbol, ctx: &QualityFamilyContext<'_>) -> String {
    let since = ctx.history.map_or_else(String::new, |h| iso_date(h.since));
    ctx.file_churn(sym).map_or_else(
        || " commits=\"0\"".to_string(),
        |c| {
            let last = iso_date(c.last_commit_epoch);
            format!(
                " commits=\"{}\" last=\"{last}\" window_since=\"{since}\"",
                c.commits
            )
        },
    )
}

const HISTORICAL_FAMILY: QualityFamilySpec = QualityFamilySpec {
    name: "historical",
    c_only: false,
    predicate: historical_fires,
    measure: historical_measure,
};

fn colocation_measure(sym: &Symbol, ctx: &QualityFamilyContext<'_>) -> String {
    format!(" out=\"{}\"", ctx.graph.out_degree(sym.id))
}

const COLOCATION_FAMILY: QualityFamilySpec = QualityFamilySpec {
    name: "colocation",
    c_only: false,
    predicate: |sym, ctx| ctx.graph.out_degree(sym.id) >= 10,
    measure: colocation_measure,
};

fn state_access(sym: &Symbol, ctx: &QualityFamilyContext<'_>) -> StateAccess {
    ctx.state_syms
        .and_then(|m| m.get(&sym.id))
        .copied()
        .unwrap_or_default()
}

fn state_measure(sym: &Symbol, ctx: &QualityFamilyContext<'_>) -> String {
    let a = state_access(sym, ctx);
    format!(" reads=\"{}\" writes=\"{}\"", a.reads, a.writes)
}

const STATE_FAMILY: QualityFamilySpec = QualityFamilySpec {
    name: "state",
    c_only: false,
    predicate: |sym, ctx| ctx.state_syms.is_some_and(|m| m.contains_key(&sym.id)),
    measure: state_measure,
};

pub static PANEL_FAMILIES: &[&dyn QualityEvidenceFamily] = &[
    &QualityFamilySpec {
        name: "structural",
        c_only: false,
        predicate: |sym, _| is_structural_base(sym) || sym.params > 5,
        measure: structural_measure,
    },
    &LEXICAL_FAMILY,
    &CONFUSION_FAMILY,
    &HISTORICAL_FAMILY,
    &COLOCATION_FAMILY,
    &STATE_FAMILY,
];

pub static ENSEMBLE_FAMILIES: &[&dyn QualityEvidenceFamily] = &[
    &QualityFamilySpec {
        name: "structural",
        c_only: false,
        predicate: |sym, _| is_structural_base(sym),
        measure: structural_measure,
    },
    &QualityFamilySpec {
        name: "colocation",
        c_only: false,
        predicate: |sym, ctx| ctx.graph.out_degree(sym.id) >= 8,
        measure: colocation_measure,
    },
    &HISTORICAL_FAMILY,
    &LEXICAL_FAMILY,
    &CONFUSION_FAMILY,
];

#[must_use]
pub fn evaluate_quality_families(
    sym: &Symbol,
    ctx: &QualityFamilyContext<'_>,
    families: &[&dyn QualityEvidenceFamily],
) -> Vec<&'static str> {
    families
        .iter()
        .filter(|fam| fam.fires(sym, ctx))
        .map(|fam| fam.name())
        .collect()
}

/// Families whose analysis is unavailable for this invocation, with the reason.
pub(crate) fn unavailable_families(
    cfiles: usize,
    history: &Result<PanelHistory, String>,
) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if cfiles == 0 {
        out.push(("confusion", "confusion: C/C++ only".to_string()));
    }
    if let Err(why) = history {
        out.push(("historical", format!("historical: {why}")));
    }
    out
}

/// Families from `all` that are evaluated for this invocation.
pub(crate) fn enabled_families(
    all: &[&'static dyn QualityEvidenceFamily],
    unavailable: &[(&'static str, String)],
) -> Vec<&'static dyn QualityEvidenceFamily> {
    all.iter()
        .copied()
        .filter(|f| !unavailable.iter().any(|(name, _)| *name == f.name()))
        .collect()
}

/// Root `families`/`enabled`/`unavailable` attributes; `total` is the number of families defined.
pub(crate) fn family_attrs(
    total: usize,
    enabled: &[&str],
    unavailable: &[(&str, String)],
) -> String {
    let mut out = format!(
        " families=\"{total}\" enabled=\"{}\" enabled_n=\"{}\"",
        enabled.join(","),
        enabled.len()
    );
    if !unavailable.is_empty() {
        let names: Vec<&str> = unavailable.iter().map(|(n, _)| *n).collect();
        let why: Vec<&str> = unavailable.iter().map(|(_, w)| w.as_str()).collect();
        out.push_str(&format!(
            " unavailable=\"{}\" unavailable_why=\"{}\"",
            names.join(","),
            why.join("; ").as_str().xml_escaped()
        ));
    }
    out
}

fn collect_nonlocal_state_info(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> (HashMap<NodeId, StateAccess>, usize, usize) {
    let funcs = crate::metrics::nonlocal::find_nonlocal_state(ingest, graph, interner);
    let cells = funcs.iter().map(|f| f.cells.len()).sum();
    let mut state_files = std::collections::HashSet::new();
    let mut state_syms: HashMap<NodeId, StateAccess> = HashMap::new();
    for f in &funcs {
        state_files.insert(f.path.as_str());
        let acc = state_syms.entry(f.sym).or_default();
        acc.reads += f.reads;
        acc.writes += f.writes;
    }
    (state_syms, state_files.len(), cells)
}

/// Inputs beyond the analysis graph: the preset and the git history behind the historical family.
pub struct PanelInputs<'a> {
    pub preset: Option<&'a str>,
    pub history: Result<PanelHistory, String>,
}

fn panel_row(
    sym: &Symbol,
    ingest: &IngestResult,
    interner: &Interner,
    enabled: &[&'static dyn QualityEvidenceFamily],
    ctx: &QualityFamilyContext<'_>,
    fired: &[&'static str],
) -> QualityPanelRow {
    let path = ingest
        .files
        .get(sym.file_id as usize)
        .map(|f| interner.resolve(&f.path))
        .unwrap_or("");
    let applicable: Vec<&dyn QualityEvidenceFamily> =
        enabled.iter().copied().filter(|f| f.applies(sym)).collect();
    let evidence = applicable
        .iter()
        .filter(|f| fired.contains(&f.name()))
        .map(|f| FamilyEvidence {
            family: f.name(),
            attrs: f.evidence(sym, ctx),
        })
        .collect();
    QualityPanelRow {
        path: path.to_string(),
        line: sym.line,
        name: interner.resolve(&sym.name).to_string(),
        fam: fired.len(),
        of: applicable.len(),
        fired: fired.join(","),
        evidence,
    }
}

pub fn compute_quality_panel_report(
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
    inputs: PanelInputs<'_>,
    page: PageRequest,
) -> QualityPanelReport {
    let preset = inputs.preset.unwrap_or("default");
    let history = inputs.history;
    let cut = usize::from(preset != "lenient") + 1;
    let (state_syms, sfiles, cells) = collect_nonlocal_state_info(ingest, graph, interner);
    let sscope = state_syms.len();
    let tested_mask = crate::navigate::compute_tested_node_mask(ingest, graph, interner);
    let cfiles = ingest
        .files
        .iter()
        .filter(|f| f.lang.semantics().supports_c_layout())
        .count();
    let unavailable = unavailable_families(cfiles, &history);
    let enabled = enabled_families(PANEL_FAMILIES, &unavailable);
    let enabled_names: Vec<&'static str> = enabled.iter().map(|f| f.name()).collect();
    let ctx = QualityFamilyContext {
        graph,
        interner,
        state_syms: Some(&state_syms),
        history: history.as_ref().ok(),
    };
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
        cscope += usize::from(sym.lang.semantics().supports_c_layout());
        let fired = evaluate_quality_families(sym, &ctx, &enabled);
        cranked += usize::from(fired.contains(&"confusion"));
        hranked += usize::from(fired.contains(&"historical"));
        let fam_count = fired.len();
        if fam_count == 0 {
            no_family += 1;
        } else if fam_count < cut {
            below_cut += 1;
        } else {
            rows.push(panel_row(sym, ingest, interner, &enabled, &ctx, &fired));
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
        enabled: enabled_names,
        unavailable,
        history,
        rows,
        page,
    }
}

impl QualityPanelReport {
    fn window(&self) -> PageWindow {
        self.page.window_fit(self.rows.len(), |i| {
            row_bytes(|w| write_quality_row(&self.rows[i], self.unavail_row_attr(), w))
        })
    }

    fn unavail_row_attr(&self) -> &'static str {
        if self.cfiles == 0 {
            " unavail=\"confusion\""
        } else {
            ""
        }
    }
}

fn write_quality_row<W: Write + ?Sized>(
    r: &QualityPanelRow,
    unavail_row_attr: &str,
    w: &mut W,
) -> io::Result<()> {
    use crate::serialize::XmlEscapeExt;
    writeln!(
        w,
        "  <s p=\"{}:{}\" n=\"{}\" fam=\"{}\" of=\"{}\" fired=\"{}\"{unavail_row_attr}>",
        r.path.xml_escaped(),
        r.line,
        r.name.xml_escaped(),
        r.fam,
        r.of,
        r.fired
    )?;
    r.evidence
        .iter()
        .try_for_each(|e| writeln!(w, "    <e f=\"{}\"{}/>", e.family, e.attrs))?;
    writeln!(w, "  </s>")
}

impl crate::serialize::XmlReport for QualityPanelReport {
    crate::xml_report_meta!(
        "quality_panel",
        "ripwire-rs.quality-panel/v1",
        root: true,
        comment: "ripwire-rs quality-panel schema=ripwire-rs.quality-panel/v1: every quality family in ONE report. state = MUTABLE non-local state only (reassignable module/class bindings, and constants the indexed files mutate); reads of unmutated configuration constants are NOT state (state_lens=mutable). historical = git commits per file in the window at or above hcut (p90 of changed files, clamped to 3..14; hbasis says which)."
    );
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let pw = self.window();
        let ccut = if self.cfiles > 0 { 40 } else { 0 };
        let history_attrs = history_root_attrs(&self.history, self.hranked);
        let family = family_attrs(PANEL_FAMILIES.len(), &self.enabled, &self.unavailable);
        write!(
            w,
            "{} preset=\"{}\" cut=\"{}\" cut_reachable=\"1\" eligible=\"{}\" ranked=\"{}\" below_cut=\"{}\" no_family=\"{}\" bar_ccx=\"15\" bar_loc=\"60\" bar_nest=\"4\" bar_params=\"5\" rcut=\"40\" rmeasured=\"{}\" hranked=\"{}\"{history_attrs} cfiles=\"{}\" cscope=\"{}\" cranked=\"{}\" ccut=\"{ccut}\" lscope=\"{}\" sfiles=\"{}\" sscope=\"{}\" state_lens=\"mutable\" cells=\"{}\" tested_scope=\"{}\" deep_untested=\"{}\" shown=\"{}\" capped=\"{}\" total=\"{}\" has_more=\"{}\" next_offset=\"{}\" offset=\"{}\" limit=\"{}\"",
            family,
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
            pw.next_offset,
            pw.offset,
            pw.limit
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let unavail_row_attr = self.unavail_row_attr();
        let pw = self.window();
        for r in &self.rows[pw.begin..pw.end] {
            write_quality_row(r, unavail_row_attr, w)?;
        }
        Ok(())
    }
}
