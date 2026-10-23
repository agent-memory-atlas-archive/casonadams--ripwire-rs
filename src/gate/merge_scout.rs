//! `--merge-scout REF1,REF2,...`: symbol-level overlap between divergent arms and HEAD.
//!
//! Each arm is compared with its merge base against HEAD. The git reads (refs, merge bases, tree
//! deltas, blob hunks) live in `git::merge_scout`; this module turns their per-file symbol sets into
//! verdicts, so it stays pure and testable in memory.
use crate::domain::SymKind;
use crate::ingest::extract::RawDef;
use crate::serialize::{PageRequest, Paged, XmlEscaped, XmlReport, row_bytes};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::ops::Range;

/// Explanation shown on an arm that has no divergent work, matching `--stray-content`'s pointer.
const NO_WORK_NOTE: &str = "no divergent work vs merge-base - see --stray-content";

/// Changed paths, each with the symbol names its changed lines fall in (empty when the file
/// changed only outside every symbol, or in a language the index does not parse).
pub type ChangedFiles = BTreeMap<String, BTreeSet<String>>;

/// A symbol named by its file and scope-qualified name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SymRef {
    pub path: String,
    pub name: String,
}

/// Git-derived inputs for one arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmChanges {
    pub label: String,
    /// Short merge-base sha with HEAD; `None` when the arm shares no history with HEAD.
    pub base: Option<String>,
    /// What the arm changed since the merge base.
    pub files: ChangedFiles,
    /// What HEAD changed since the same merge base.
    pub head_files: ChangedFiles,
}

/// One arm's verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoutArm {
    pub branch: String,
    pub base: Option<String>,
    pub files: usize,
    pub symbols: Vec<SymRef>,
    /// Symbols this arm changed that HEAD also changed since the merge base.
    pub head_conflicts: usize,
}

/// Two arms with divergent work: symbols both changed, and files both changed without a shared symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoutPair {
    pub arm_a: String,
    pub arm_b: String,
    pub conflicts: Vec<SymRef>,
    pub risks: usize,
}

/// Merge scout report across the requested arms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeScoutReport {
    pub head: String,
    pub at: String,
    pub arms: Vec<ScoutArm>,
    pub pairs: Vec<ScoutPair>,
    pub landing_order: Vec<String>,
}

fn attributable(kind: SymKind) -> bool {
    matches!(
        kind,
        SymKind::Function
            | SymKind::Method
            | SymKind::Class
            | SymKind::Struct
            | SymKind::Interface
            | SymKind::Macro
            | SymKind::Section
    )
}

fn qualified_name(def: &RawDef) -> String {
    match &def.scope {
        Some(scope) => format!("{scope}::{}", def.name),
        None => def.name.clone(),
    }
}

/// Byte offset at which each line of `source` starts; line `k` spans `starts[k]..starts[k + 1]`.
fn line_starts(source: &[u8]) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            source
                .iter()
                .enumerate()
                .filter(|(_, byte)| **byte == b'\n')
                .map(|(i, _)| i + 1),
        )
        .collect()
}

fn line_of(starts: &[usize], byte: usize) -> usize {
    starts.partition_point(|&s| s <= byte) - 1
}

/// Names of the symbols that own a changed line.
///
/// Every changed line (0-based, half-open ranges) belongs to its innermost attributable definition,
/// so editing a method marks that method and not the enclosing `impl`. Locals, fields and module
/// constants are not attributable: a line that only they own names no symbol.
#[must_use]
pub fn touched_symbol_names(
    source: &[u8],
    defs: &[RawDef],
    changed: &[Range<u32>],
) -> BTreeSet<String> {
    let starts = line_starts(source);
    let lines = starts.len();
    let mut spans: Vec<(usize, usize, &RawDef)> = defs
        .iter()
        .filter(|d| attributable(d.kind))
        .map(|d| {
            let first = line_of(&starts, d.start_byte);
            let last = line_of(&starts, d.end_byte.max(d.start_byte + 1) - 1);
            (first, last, d)
        })
        .collect();
    // Outermost first, so a nested definition painted later wins its own lines.
    spans.sort_by_key(|&(first, last, _)| std::cmp::Reverse(last - first));
    let mut owner: Vec<Option<&RawDef>> = vec![None; lines];
    for &(first, last, def) in &spans {
        for slot in &mut owner[first..=last.min(lines - 1)] {
            *slot = Some(def);
        }
    }
    let mut names = BTreeSet::new();
    for range in changed {
        let end = (range.end as usize).min(lines);
        let start = (range.start as usize).min(end);
        for def in owner[start..end].iter().flatten() {
            names.insert(qualified_name(def));
        }
    }
    names
}

fn sym_refs(files: &ChangedFiles) -> Vec<SymRef> {
    files
        .iter()
        .flat_map(|(path, names)| {
            names.iter().map(move |name| SymRef {
                path: path.clone(),
                name: name.clone(),
            })
        })
        .collect()
}

fn scout_arm(arm: &ArmChanges) -> ScoutArm {
    let symbols = sym_refs(&arm.files);
    let head_conflicts = symbols
        .iter()
        .filter(|s| {
            arm.head_files
                .get(&s.path)
                .is_some_and(|names| names.contains(&s.name))
        })
        .count();
    ScoutArm {
        branch: arm.label.clone(),
        base: arm.base.clone(),
        files: arm.files.len(),
        symbols,
        head_conflicts,
    }
}

fn scout_pair(a: &ArmChanges, b: &ArmChanges) -> ScoutPair {
    let mut conflicts = Vec::new();
    let mut risks = 0;
    for (path, names_a) in &a.files {
        let Some(names_b) = b.files.get(path) else {
            continue;
        };
        let shared: Vec<SymRef> = names_a
            .intersection(names_b)
            .map(|name| SymRef {
                path: path.clone(),
                name: name.clone(),
            })
            .collect();
        if shared.is_empty() {
            risks += 1;
        } else {
            conflicts.extend(shared);
        }
    }
    ScoutPair {
        arm_a: a.label.clone(),
        arm_b: b.label.clone(),
        conflicts,
        risks,
    }
}

/// Verdicts for `arms`, in request order. Pairs cover every arm that ran, work or not, as upstream
/// does; the landing order covers only arms with work.
///
/// Landing order covers only arms with work (a merge base reached and at least one changed path),
/// fewest conflicts first: a conflict is a symbol the arm shares with HEAD or with another arm. Ties
/// go to the smaller change (symbols, then files), then to request order.
#[must_use]
pub fn evaluate_merge_scout(head: &str, at: &str, arms: &[ArmChanges]) -> MergeScoutReport {
    let scout: Vec<ScoutArm> = arms.iter().map(scout_arm).collect();
    let ran: Vec<usize> = (0..arms.len())
        .filter(|&i| arms[i].base.is_some())
        .collect();
    let working: Vec<usize> = ran
        .iter()
        .copied()
        .filter(|&i| !arms[i].files.is_empty())
        .collect();
    let mut load: Vec<usize> = scout.iter().map(|a| a.head_conflicts).collect();
    let mut pairs = Vec::new();
    for (k, &i) in ran.iter().enumerate() {
        for &j in &ran[k + 1..] {
            let pair = scout_pair(&arms[i], &arms[j]);
            load[i] += pair.conflicts.len();
            load[j] += pair.conflicts.len();
            pairs.push(pair);
        }
    }
    let mut order = working;
    order.sort_by_key(|&i| (load[i], scout[i].symbols.len(), scout[i].files, i));
    MergeScoutReport {
        head: head.to_string(),
        at: at.to_string(),
        arms: scout,
        pairs,
        landing_order: order.iter().map(|&i| arms[i].label.clone()).collect(),
    }
}

fn write_sym<W: Write + ?Sized>(sym: &SymRef, out: &mut W) -> io::Result<()> {
    writeln!(
        out,
        "    <sym p=\"{}\" n=\"{}\"/>",
        XmlEscaped(&sym.path),
        XmlEscaped(&sym.name)
    )
}

/// Symbol rows of one arm or pair, windowed by the page budget; the window is returned so the
/// enclosing element can name what it shows.
fn sym_rows<'a>(syms: &'a [SymRef]) -> Paged<'a, SymRef> {
    Paged::new(syms, PageRequest::default(), |s| {
        row_bytes(|w| write_sym(s, w))
    })
}

fn write_arm<W: Write + ?Sized>(arm: &ScoutArm, out: &mut W) -> io::Result<()> {
    let (Some(base), files) = (&arm.base, arm.files) else {
        return writeln!(
            out,
            "  <arm ref=\"{}\" ok=\"0\" note=\"no merge-base with HEAD: arm not compared\"/>",
            XmlEscaped(&arm.branch)
        );
    };
    if files == 0 {
        return writeln!(
            out,
            "  <arm ref=\"{}\" base=\"{base}\" ok=\"1\" changed=\"0\" head_conflicts=\"0\"><no-work note=\"{NO_WORK_NOTE}\"/></arm>",
            XmlEscaped(&arm.branch)
        );
    }
    let window = sym_rows(&arm.symbols).window;
    write!(
        out,
        "  <arm ref=\"{}\" base=\"{base}\" ok=\"1\" changed=\"{}\" head_conflicts=\"{}\" files=\"{files}\"",
        XmlEscaped(&arm.branch),
        arm.symbols.len(),
        arm.head_conflicts,
    )?;
    if window.capped {
        write!(out, " shown=\"{}\" capped=\"1\"", window.shown)?;
    }
    writeln!(out, ">")?;
    for sym in sym_rows(&arm.symbols).rows() {
        write_sym(sym, out)?;
    }
    writeln!(out, "  </arm>")
}

fn write_pair<W: Write + ?Sized>(pair: &ScoutPair, out: &mut W) -> io::Result<()> {
    if pair.conflicts.is_empty() {
        return writeln!(
            out,
            "  <pair a=\"{}\" b=\"{}\" conflicts=\"0\" risks=\"{}\"/>",
            XmlEscaped(&pair.arm_a),
            XmlEscaped(&pair.arm_b),
            pair.risks
        );
    }
    let window = sym_rows(&pair.conflicts).window;
    write!(
        out,
        "  <pair a=\"{}\" b=\"{}\" conflicts=\"{}\" risks=\"{}\"",
        XmlEscaped(&pair.arm_a),
        XmlEscaped(&pair.arm_b),
        pair.conflicts.len(),
        pair.risks
    )?;
    if window.capped {
        write!(out, " shown=\"{}\" capped=\"1\"", window.shown)?;
    }
    writeln!(out, ">")?;
    for sym in sym_rows(&pair.conflicts).rows() {
        write_sym(sym, out)?;
    }
    writeln!(out, "  </pair>")
}

impl XmlReport for MergeScoutReport {
    crate::xml_report_meta!(
        "merge-scout",
        "ripwire-rs.merge-scout/v1",
        root: false,
        comment: "ripwire-rs merge-scout schema=ripwire-rs.merge-scout/v1: symbol overlap of arms= refs vs HEAD (head=, at=). arm: ok=1 means compared; changed= symbols the arm changed since its merge base; head_conflicts= of those, symbols HEAD also changed since that base; no-work= nothing divergent. pair: conflicts= symbols both arms changed (listed as sym), risks= files both changed without a shared symbol. landing: order of the arms with work, fewest conflicts first."
    );
    fn write_extra_attrs<W: Write>(&self, w: &mut W) -> io::Result<()> {
        write!(
            w,
            " arms=\"{}\" head=\"{}\" at=\"{}\"",
            self.arms.len(),
            self.head,
            self.at
        )
    }
    fn write_body<W: Write>(&self, w: &mut W) -> io::Result<()> {
        for arm in &self.arms {
            write_arm(arm, w)?;
        }
        for pair in &self.pairs {
            write_pair(pair, w)?;
        }
        writeln!(w, "  <landing order=\"{}\"/>", self.landing_order.join(","))
    }
}

/// Serialize merge scout report to canonical XML format.
pub fn serialize_merge_scout_xml<W: Write>(rep: &MergeScoutReport, out: &mut W) -> io::Result<()> {
    rep.render_envelope(out)
}
