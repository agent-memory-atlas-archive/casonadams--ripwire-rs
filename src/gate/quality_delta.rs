use crate::domain::{IngestResult, Interner};
use crate::gate::baseline::{
    DEFAULT_BASELINE_FILE, QualitySnapshot, compute_snapshot, read_baseline_file,
};
use crate::mcp::McpState;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;

pub const DEFAULT_ACKS_FILE: &str = ".ripwire_quality_acks";

/// Acknowledged quality regressions from `.ripwire_quality_acks`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckEntry {
    pub kind: String,
    pub key: String,
    pub val: u32,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleAck {
    pub kind: String,
    pub key: String,
    pub why: &'static str,
    pub sym: String,
    pub path: String,
}

#[derive(Debug, Clone, Default)]
pub struct QualityAcks {
    pub acks: HashMap<(String, String), u32>,
    pub entries: Vec<AckEntry>,
}

impl QualityAcks {
    pub fn is_acknowledged(&self, kind: &str, key: &str, now_val: u32) -> bool {
        let check_val = |ack_val: u32| -> bool {
            ack_val >= now_val
                || ack_val == 0
                || kind == "short-horizon-churn"
                || ack_val.saturating_add(12) >= now_val
        };
        if let Some(&ack_val) = self.acks.get(&(kind.to_string(), key.to_string())) {
            return check_val(ack_val);
        }
        let sym = key.split("::").last().unwrap_or(key);
        if let Some(&ack_val) = self.acks.get(&(kind.to_string(), sym.to_string())) {
            return check_val(ack_val);
        }
        false
    }
}

fn insert_ack_alias(acks: &mut HashMap<(String, String), u32>, kind: &str, key: &str, val: u32) {
    acks.entry((kind.to_string(), key.to_string()))
        .and_modify(|v| *v = (*v).max(val))
        .or_insert(val);
}

pub fn parse_acks_content(content: &str) -> QualityAcks {
    let mut acks = HashMap::new();
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 4 && parts[0] == "ack" {
            let kind = parts[1].split(':').next().unwrap_or(parts[1]).to_string();
            let key = parts[2].to_string();
            let val = parts[3].parse::<u32>().unwrap_or(0);
            insert_ack_alias(&mut acks, &kind, &key, val);
            let mut path_opt = None;
            for p in &parts[4..] {
                if let Some(path_part) = p.strip_prefix("p=") {
                    path_opt = Some(path_part.to_string());
                    let sym_or_path = path_part.split(':').next().unwrap_or(path_part);
                    insert_ack_alias(&mut acks, &kind, sym_or_path, val);
                    insert_ack_alias(&mut acks, &kind, path_part, val);
                }
            }
            entries.push(AckEntry {
                kind,
                key,
                val,
                path: path_opt,
            });
        }
    }
    QualityAcks { acks, entries }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityRegression {
    pub kind: String,
    pub sym_name: String,
    pub file_path: String,
    pub line: u32,
    pub was_val: u32,
    pub now_val: u32,
    pub is_gating: bool,
    pub is_new_symbol: bool,
}

/// Comprehensive delta report across all quality dimensions.
pub struct QualityDeltaReport {
    pub baseline_source: String,
    pub head_sha: String,
    pub at_stamp: String,
    pub head_basis: Option<&'static str>,
    pub regressions: Vec<QualityRegression>,
    pub stale_acks: Vec<StaleAck>,
    pub acked_count: usize,
    pub gating_count: usize,
    pub preexisting_worse_count: usize,
    pub new_symbol_count: usize,
    pub minor_count: usize,
    pub api_new_surface: usize,
    pub stale_collapse_replay: Option<String>,
    pub baseline_absorbed: usize,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DeltaCliOptions<'a> {
    pub scope: Option<&'a str>,
    pub quality_ack: Option<&'a str>,
    pub ack_only: Option<&'a str>,
    pub show_stale: bool,
}

#[must_use]
pub fn stale_by_kind(stale: &[StaleAck]) -> String {
    let mut by_kind: Vec<(&str, usize)> = Vec::new();
    for sa in stale {
        let kind = sa.kind.split(':').next().unwrap_or(&sa.kind);
        match by_kind.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => by_kind.push((kind, 1)),
        }
    }
    by_kind
        .iter()
        .map(|(k, n)| format!("{k}:{n}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn write_regression_row<W: Write>(r: &QualityRegression, out: &mut W) -> io::Result<()> {
    let origin_attr = if r.is_new_symbol {
        " origin=\"new-symbol\""
    } else {
        ""
    };
    let gating_val = usize::from(r.is_gating);
    let bar_val = match r.kind.as_str() {
        "complexity" => 15,
        "verbosity" => 60,
        "nesting" => 4,
        "params" => 5,
        _ => 0,
    };
    writeln!(
        out,
        "  <r kind=\"{}\" sym=\"{}\" p=\"{}:{}\" was=\"{}\" now=\"{}\" gating=\"{}\" bar=\"{bar_val}\"{}/>",
        r.kind, r.sym_name, r.file_path, r.line, r.was_val, r.now_val, gating_val, origin_attr
    )
}

impl QualityDeltaReport {
    fn collapsed_stale_replay(&self) -> Option<&str> {
        let eligible = self.regressions.is_empty() && !self.stale_acks.is_empty();
        self.stale_collapse_replay.as_deref().filter(|_| eligible)
    }
}

impl crate::serialize::ToXmlString for QualityDeltaReport {
    fn write_xml<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let mut hb_attr = match self.head_basis {
            Some(hb) => format!(" head_basis=\"{hb}\""),
            None => String::new(),
        };
        if self.baseline_absorbed > 0 {
            hb_attr.push_str(&format!(
                " baseline_absorbed=\"{}\"",
                self.baseline_absorbed
            ));
        }
        let collapse = self.collapsed_stale_replay();
        let stale_attr = collapse.map_or_else(String::new, |replay| {
            use crate::serialize::XmlEscapeExt;
            format!(
                " stale_by_kind=\"{}\" stale_next=\"{} --show-stale\"",
                stale_by_kind(&self.stale_acks).xml_escaped(),
                replay.xml_escaped()
            )
        });
        let verdict_only = collapse.is_some()
            && self.head_basis == Some("identity")
            && self.acked_count == 0
            && self.baseline_absorbed == 0
            && self.api_new_surface == 0;
        if verdict_only {
            return writeln!(
                out,
                "<!-- ripwire quality-delta schema=ripwire.quality-delta/v1: only what the change made WORSE vs baseline=: regressions= minor= gating=; <r kind= sym= p= was= now= gating= bar=>, <sa> acked; with no regressions the <sa> rows collapse to stale_by_kind= and stale_next= prints them. -->\n<quality-delta schema=\"ripwire.quality-delta/v1\" baseline=\"{}\" regressions=\"0\" gating=\"0\" stale=\"{}\" at=\"{}\"{hb_attr}{stale_attr}></quality-delta>",
                self.baseline_source,
                self.stale_acks.len(),
                self.at_stamp
            );
        }
        writeln!(
            out,
            "<!-- ripwire quality-delta schema=ripwire.quality-delta/v1: only what the change made WORSE vs baseline=: regressions= minor= gating=; <r kind= sym= p= was= now= gating= bar=>, <sa> acked; with no regressions the <sa> rows collapse to stale_by_kind= and stale_next= prints them. baseline_absorbed=N: the pinned sidecar was taken on a dirty tree that already held N gating findings, so a green exit means clean since the pin. -->\n<quality-delta schema=\"ripwire.quality-delta/v1\" baseline=\"{}\" regressions=\"{}\" minor=\"{}\" acked=\"{}\" stale=\"{}\" preexisting-worse=\"{}\" new-symbol=\"{}\" gating=\"{}\" register-macro-excluded=\"0\" api-new-surface=\"{}\" at=\"{}\" renames=\"0\" rename_window_commits=\"0\" acked_by_rename=\"0\" acked_by_content=\"0\"{hb_attr}{stale_attr}>",
            self.baseline_source,
            self.regressions.len(),
            self.minor_count,
            self.acked_count,
            self.stale_acks.len(),
            self.preexisting_worse_count,
            self.new_symbol_count,
            self.gating_count,
            self.api_new_surface,
            self.at_stamp
        )?;
        for r in &self.regressions {
            write_regression_row(r, out)?;
        }
        if collapse.is_none() {
            for sa in &self.stale_acks {
                writeln!(
                    out,
                    "  <sa kind=\"{}\" key=\"{}\" why=\"{}\" sym=\"{}\" p=\"{}\"/>",
                    sa.kind, sa.key, sa.why, sa.sym, sa.path
                )?;
            }
        }
        writeln!(out, "</quality-delta>")
    }
}

impl QualityDeltaReport {
    /// True if there are major unacknowledged regressions requiring failure exit code.
    #[must_use]
    pub fn has_gating_failures(&self) -> bool {
        self.gating_count > 0
    }
}

/// Trait encapsulating a numeric quality regression dimension compared between baseline and working snapshots.
pub trait QualityDeltaDimension: Sync {
    fn kind(&self) -> &'static str;
    fn extract(&self, snap: &QualitySnapshot, key: &str) -> u32;
    fn preexisting_floor(&self) -> u32;
    fn is_preexisting_gating(&self, now: u32) -> bool;
    fn new_symbol_floor(&self) -> Option<u32> {
        None
    }
}

struct ComplexityDimension;
impl QualityDeltaDimension for ComplexityDimension {
    fn kind(&self) -> &'static str {
        "complexity"
    }
    fn extract(&self, snap: &QualitySnapshot, key: &str) -> u32 {
        snap.ccx_by_sym.get(key).copied().unwrap_or(0)
    }
    fn preexisting_floor(&self) -> u32 {
        0
    }
    fn is_preexisting_gating(&self, now: u32) -> bool {
        now > 15
    }
    fn new_symbol_floor(&self) -> Option<u32> {
        Some(15)
    }
}

struct VerbosityDimension;
impl QualityDeltaDimension for VerbosityDimension {
    fn kind(&self) -> &'static str {
        "verbosity"
    }
    fn extract(&self, snap: &QualitySnapshot, key: &str) -> u32 {
        snap.loc_by_sym.get(key).copied().unwrap_or(0)
    }
    fn preexisting_floor(&self) -> u32 {
        60
    }
    fn is_preexisting_gating(&self, _now: u32) -> bool {
        false
    }
}

struct NestingDimension;
impl QualityDeltaDimension for NestingDimension {
    fn kind(&self) -> &'static str {
        "nesting"
    }
    fn extract(&self, snap: &QualitySnapshot, key: &str) -> u32 {
        u32::from(snap.nest_by_sym.get(key).copied().unwrap_or(0))
    }
    fn preexisting_floor(&self) -> u32 {
        4
    }
    fn is_preexisting_gating(&self, now: u32) -> bool {
        now > 6
    }
}

struct ParamsDimension;
impl QualityDeltaDimension for ParamsDimension {
    fn kind(&self) -> &'static str {
        "params"
    }
    fn extract(&self, snap: &QualitySnapshot, key: &str) -> u32 {
        u32::from(snap.params_by_sym.get(key).copied().unwrap_or(0))
    }
    fn preexisting_floor(&self) -> u32 {
        3
    }
    fn is_preexisting_gating(&self, now: u32) -> bool {
        now > 5
    }
}

pub static DELTA_DIMENSIONS: &[&dyn QualityDeltaDimension] = &[
    &ComplexityDimension,
    &VerbosityDimension,
    &NestingDimension,
    &ParamsDimension,
];

fn check_symbol_numeric_regressions(
    key: &str,
    working: &QualitySnapshot,
    baseline: &QualitySnapshot,
    is_preexisting: bool,
    regressions: &mut Vec<QualityRegression>,
) {
    let parts: Vec<&str> = key.split("::").collect();
    let file_path = parts.first().copied().unwrap_or(key).to_string();
    let sym_name = parts.last().copied().unwrap_or(key).to_string();
    let line = working.line_by_sym.get(key).copied().unwrap_or(0);

    for dim in DELTA_DIMENSIONS {
        let now_val = dim.extract(working, key);
        if is_preexisting {
            let was_val = dim.extract(baseline, key);
            if now_val > was_val && now_val > dim.preexisting_floor() {
                regressions.push(QualityRegression {
                    kind: dim.kind().to_string(),
                    sym_name: sym_name.clone(),
                    file_path: file_path.clone(),
                    line,
                    was_val,
                    now_val,
                    is_gating: dim.is_preexisting_gating(now_val),
                    is_new_symbol: false,
                });
            }
        } else if dim.new_symbol_floor().is_some_and(|floor| now_val > floor) {
            regressions.push(QualityRegression {
                kind: dim.kind().to_string(),
                sym_name: sym_name.clone(),
                file_path: file_path.clone(),
                line,
                was_val: 0,
                now_val,
                is_gating: false,
                is_new_symbol: true,
            });
        }
    }
}

fn check_structural_regressions(
    working: &QualitySnapshot,
    baseline: &QualitySnapshot,
    regressions: &mut Vec<QualityRegression>,
) {
    for dead in &working.dead_symbols {
        if !baseline.dead_symbols.contains(dead) {
            let is_new = !baseline.ccx_by_sym.contains_key(dead);
            regressions.push(QualityRegression {
                kind: "dead-code".to_string(),
                sym_name: dead.clone(),
                file_path: dead.split("::").next().unwrap_or(dead).to_string(),
                line: working.line_by_sym.get(dead).copied().unwrap_or(0),
                was_val: 0,
                now_val: 1,
                is_gating: !is_new,
                is_new_symbol: is_new,
            });
        }
    }
    for pub_sym in working.public_api.difference(&baseline.public_api) {
        regressions.push(QualityRegression {
            kind: "api-surface".to_string(),
            sym_name: pub_sym.split("::").last().unwrap_or(pub_sym).to_string(),
            file_path: pub_sym.split("::").next().unwrap_or(pub_sym).to_string(),
            line: working.line_by_sym.get(pub_sym).copied().unwrap_or(0),
            was_val: 0,
            now_val: 1,
            is_gating: false,
            is_new_symbol: true,
        });
    }
}

/// Compute quality delta report comparing working tree snapshot against baseline.
#[must_use]
pub fn compute_quality_delta(
    working: &QualitySnapshot,
    baseline: &QualitySnapshot,
    baseline_source: &str,
) -> QualityDeltaReport {
    compute_quality_delta_with_acks(working, baseline, baseline_source, None)
}

fn apply_quality_acks(regressions: &mut [QualityRegression], ledger: &QualityAcks) -> usize {
    let mut acked = 0usize;
    for r in regressions {
        let file = r.file_path.split(':').next().unwrap_or(&r.file_path);
        if ledger.is_acknowledged(&r.kind, &r.sym_name, r.now_val)
            || ledger.is_acknowledged(&r.kind, &r.file_path, r.now_val)
            || ledger.is_acknowledged(&r.kind, file, r.now_val)
        {
            if r.is_gating {
                acked += 1;
            }
            r.is_gating = false;
        }
    }
    acked
}

fn collect_stale_acks(ledger: &QualityAcks, regressions: &[QualityRegression]) -> Vec<StaleAck> {
    let mut out = Vec::new();
    for entry in &ledger.entries {
        let active = regressions.iter().any(|r| {
            r.kind == entry.kind
                && (r.sym_name == entry.key
                    || entry
                        .path
                        .as_deref()
                        .is_some_and(|p| p.starts_with(&r.file_path)))
        });
        if !active {
            out.push(StaleAck {
                kind: entry.kind.clone(),
                key: entry.key.clone(),
                why: "finding-gone",
                sym: entry.key.clone(),
                path: entry
                    .path
                    .clone()
                    .unwrap_or_else(|| "src/lib.rs:1".to_string()),
            });
        }
    }
    out
}

fn count_delta_categories(regressions: &[QualityRegression]) -> (usize, usize, usize) {
    let mut gating = 0;
    let mut preexisting = 0;
    let mut new_sym = 0;
    for r in regressions {
        if r.is_gating {
            gating += 1;
        }
        if r.is_new_symbol {
            new_sym += 1;
        } else {
            preexisting += 1;
        }
    }
    (gating, preexisting, new_sym)
}

/// Compute quality delta report comparing working tree snapshot against baseline with acks.
#[must_use]
pub fn compute_quality_delta_with_acks(
    working: &QualitySnapshot,
    baseline: &QualitySnapshot,
    baseline_source: &str,
    acks: Option<&QualityAcks>,
) -> QualityDeltaReport {
    let mut regressions = Vec::new();

    for key in working.ccx_by_sym.keys() {
        let is_preexisting = baseline.ccx_by_sym.contains_key(key);
        check_symbol_numeric_regressions(key, working, baseline, is_preexisting, &mut regressions);
    }

    check_structural_regressions(working, baseline, &mut regressions);

    let (acked_count, stale_acks) = match acks {
        Some(ledger) => (
            apply_quality_acks(&mut regressions, ledger),
            collect_stale_acks(ledger, &regressions),
        ),
        None => (0, Vec::new()),
    };
    let (gating_count, preexisting_worse_count, new_symbol_count) =
        count_delta_categories(&regressions);
    let api_new_surface = working.public_api.difference(&baseline.public_api).count();
    let short_sha = if working.head_sha.len() >= 9 {
        &working.head_sha[..9]
    } else {
        &working.head_sha
    };
    let at_stamp = if working.dirty {
        format!("{short_sha}+dirty")
    } else {
        short_sha.to_string()
    };
    let minor_count = regressions.len().saturating_sub(gating_count);

    QualityDeltaReport {
        baseline_source: baseline_source.to_string(),
        head_sha: working.head_sha.clone(),
        at_stamp,
        head_basis: None,
        regressions,
        stale_acks,
        acked_count,
        gating_count,
        preexisting_worse_count,
        new_symbol_count,
        minor_count,
        api_new_surface,
        stale_collapse_replay: None,
        baseline_absorbed: 0,
    }
}

fn remove_file_entries_from_snapshot(snap: &mut QualitySnapshot, file_prefix: &str) {
    let prefix = format!("{file_prefix}::");
    snap.ccx_by_sym.retain(|k, _| !k.starts_with(&prefix));
    snap.cx_by_sym.retain(|k, _| !k.starts_with(&prefix));
    snap.loc_by_sym.retain(|k, _| !k.starts_with(&prefix));
    snap.nest_by_sym.retain(|k, _| !k.starts_with(&prefix));
    snap.params_by_sym.retain(|k, _| !k.starts_with(&prefix));
    snap.dead_symbols.retain(|k| !k.starts_with(&prefix));
    snap.public_api.retain(|k| !k.starts_with(&prefix));
}

fn merge_snapshot_entries(dst: &mut QualitySnapshot, src: QualitySnapshot) {
    dst.ccx_by_sym.extend(src.ccx_by_sym);
    dst.cx_by_sym.extend(src.cx_by_sym);
    dst.loc_by_sym.extend(src.loc_by_sym);
    dst.nest_by_sym.extend(src.nest_by_sym);
    dst.params_by_sym.extend(src.params_by_sym);
    dst.dead_symbols.extend(src.dead_symbols);
    dst.public_api.extend(src.public_api);
}

fn read_changed_head_blobs<P: AsRef<Path>>(
    root_path: P,
    state: &McpState,
    changed_set: &std::collections::HashSet<&str>,
) -> Vec<(String, crate::domain::Lang, Vec<u8>)> {
    let mut head_files = Vec::new();
    let Ok(repo) = gix::open(root_path.as_ref()) else {
        return head_files;
    };
    let Ok(head) = repo.head_commit() else {
        return head_files;
    };
    let Ok(tree) = head.tree() else {
        return head_files;
    };
    for f in &state.ingest.files {
        let rel_path = state.interner.resolve(&f.path);
        if !changed_set.contains(rel_path) {
            continue;
        }
        if let Ok(Some(entry)) = tree.lookup_entry_by_path(rel_path)
            && let Ok(obj) = entry.object()
        {
            head_files.push((rel_path.to_string(), f.lang, obj.data.to_vec()));
        }
    }
    head_files
}

fn load_head_baseline_incremental(
    root: &str,
    state: &McpState,
    working: &QualitySnapshot,
    changed_files: &[String],
    head_sha: &str,
) -> QualitySnapshot {
    let mut snap = working.clone();
    snap.head_sha = head_sha.to_string();
    let changed_set: std::collections::HashSet<&str> =
        changed_files.iter().map(String::as_str).collect();
    for cf in &changed_set {
        remove_file_entries_from_snapshot(&mut snap, cf);
    }
    let head_files = read_changed_head_blobs(root, state, &changed_set);
    if head_files.is_empty() {
        return snap;
    }
    let head_interner = crate::domain::Interner::new();
    let Ok(registry) = crate::ingest::LanguageRegistry::new() else {
        return snap;
    };
    let slices: Vec<(&str, crate::domain::Lang, &[u8])> = head_files
        .iter()
        .map(|(p, l, b)| (p.as_str(), *l, b.as_slice()))
        .collect();
    let head_ingest = crate::ingest::ingest_in_memory_files(&slices, &registry, &head_interner);
    let head_graph = crate::graph::Graph::build(&head_ingest, &head_interner);
    let mut delta_snap = compute_snapshot(&head_ingest, &head_graph, &head_interner, head_sha);
    for k in delta_snap.ccx_by_sym.keys() {
        if working.dead_symbols.contains(k) {
            delta_snap.dead_symbols.insert(k.clone());
        }
    }
    merge_snapshot_entries(&mut snap, delta_snap);
    snap
}

fn matches_path(path: &str, file: &str) -> bool {
    path == file || path.ends_with(file)
}

fn symbol_matches_and_changed(
    sym: &crate::domain::Symbol,
    file_path: &str,
    ingest: &IngestResult,
    interner: &Interner,
    baseline: &QualitySnapshot,
) -> Option<(String, String)> {
    let sym_file = interner.resolve(&ingest.files[sym.file_id as usize].path);
    if !matches_path(sym_file, file_path) {
        return None;
    }
    let sym_name = interner.resolve(&sym.name);
    let key = match sym.scope {
        Some(sc) => format!("{sym_file}::{}::{sym_name}", interner.resolve(&sc)),
        None => format!("{sym_file}::{sym_name}"),
    };
    let &was_ccx = baseline.ccx_by_sym.get(&key)?;
    let was_loc = baseline.loc_by_sym.get(&key).copied().unwrap_or(0);
    if sym.ccx != was_ccx || sym.loc != was_loc {
        Some((sym_name.to_string(), sym_file.to_string()))
    } else {
        None
    }
}

fn find_preexisting_callable_in_file<'a>(
    file_path: &str,
    state: &'a McpState,
    baseline: &QualitySnapshot,
) -> Option<(&'a crate::domain::Symbol, String, String)> {
    for sym in &state.ingest.symbols {
        if (sym.kind == crate::domain::SymKind::Function
            || sym.kind == crate::domain::SymKind::Method)
            && let Some((sym_name, sym_file)) =
                symbol_matches_and_changed(sym, file_path, &state.ingest, &state.interner, baseline)
        {
            return Some((sym, sym_name, sym_file));
        }
    }
    None
}

fn check_churn_regressions(
    root: &str,
    state: &McpState,
    baseline: &QualitySnapshot,
    regressions: &mut Vec<QualityRegression>,
) {
    let Some(commits) = &state.commits else {
        return;
    };
    if commits.is_empty() {
        return;
    }

    let changed_files = crate::cli::safety::get_working_tree_changed_files(root);
    for f in &changed_files {
        let commit_count = commits
            .iter()
            .filter(|c| c.changed_files.iter().any(|cf| cf == f || cf.ends_with(f)))
            .count();
        if commit_count >= 2
            && let Some((sym, sym_name, sym_file)) =
                find_preexisting_callable_in_file(f, state, baseline)
        {
            regressions.push(QualityRegression {
                kind: "short-horizon-churn".to_string(),
                sym_name,
                file_path: sym_file,
                line: sym.line,
                was_val: 0,
                now_val: commit_count as u32,
                is_gating: false,
                is_new_symbol: false,
            });
        }
    }
}

/// Body-text regression kinds read from code (and, for placeholder markers, comments) only.
#[must_use]
pub fn body_text_kinds(masked: &crate::metrics::source_mask::MaskedSource) -> Vec<&'static str> {
    let code = &masked.code;
    let compact: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let mut kinds = Vec::new();
    let code_placeholder = ["todo!(", "unimplemented!(", "NotImplementedError"]
        .iter()
        .any(|m| code.contains(m));
    let comment_placeholder = ["TODO:", "FIXME:"]
        .iter()
        .any(|m| masked.comments.contains(m));
    if code_placeholder || comment_placeholder {
        kinds.push("placeholder");
    }
    if [
        "catch{}",
        "catch(_){}",
        "catch(...){}",
        "except:pass",
        ".catch(()=>{})",
    ]
    .iter()
    .any(|m| compact.contains(m))
    {
        kinds.push("error-masking");
    }
    if code.contains("std::unique(") && !code.contains("std::sort(") {
        kinds.push("defect-shape");
    }
    kinds
}

fn check_body_text_regressions(
    sym: &crate::domain::Symbol,
    sym_name: &str,
    file_path: &str,
    body: &str,
    is_new: bool,
    regressions: &mut Vec<QualityRegression>,
) {
    let masked = crate::metrics::source_mask::mask_source(body, sym.lang);
    for kind in body_text_kinds(&masked) {
        regressions.push(QualityRegression {
            kind: kind.to_string(),
            sym_name: sym_name.to_string(),
            file_path: file_path.to_string(),
            line: sym.line,
            was_val: 0,
            now_val: 1,
            is_gating: false,
            is_new_symbol: is_new,
        });
    }
}

fn collect_changed_clone_regressions(
    clones: Vec<crate::clones::CloneGroup>,
    state: &McpState,
    changed_files: &[String],
    regressions: &mut Vec<QualityRegression>,
) {
    for cg in clones {
        let Some(first_sym) = cg
            .members
            .first()
            .and_then(|&id| state.ingest.symbols.get(id as usize))
        else {
            continue;
        };
        let fp = state
            .ingest
            .files
            .get(first_sym.file_id as usize)
            .map_or("", |f| state.interner.resolve(&f.path));
        if !changed_files.iter().any(|cf| fp == cf || fp.ends_with(cf)) {
            continue;
        }
        let has_reused_helper = cg.members.iter().any(|&id| state.graph.in_degree(id) >= 2);
        let kind = if has_reused_helper {
            "new-clone-of-reused-helper"
        } else {
            "duplication"
        };
        regressions.push(QualityRegression {
            kind: kind.to_string(),
            sym_name: state.interner.resolve(&first_sym.name).to_string(),
            file_path: fp.to_string(),
            line: first_sym.line,
            was_val: 0,
            now_val: cg.token_count as u32,
            is_gating: false,
            is_new_symbol: true,
        });
    }
}

fn is_changed_callable(
    sym: &crate::domain::Symbol,
    file_path: &str,
    changed_files: &[String],
) -> bool {
    (sym.kind == crate::domain::SymKind::Function || sym.kind == crate::domain::SymKind::Method)
        && changed_files
            .iter()
            .any(|cf| file_path == cf || file_path.ends_with(cf))
}

fn check_source_and_clone_regressions(
    root: &str,
    state: &McpState,
    baseline: &QualitySnapshot,
    changed_files: &[String],
    regressions: &mut Vec<QualityRegression>,
) {
    let root_path = Path::new(root);
    for sym in &state.ingest.symbols {
        let file_path = state
            .ingest
            .files
            .get(sym.file_id as usize)
            .map_or("", |f| state.interner.resolve(&f.path));
        if is_changed_callable(sym, file_path, changed_files)
            && let Ok(bytes) =
                std::fs::read(root_path.join(file_path)).or_else(|_| std::fs::read(file_path))
        {
            let start = (sym.sig_start_byte as usize).min(bytes.len());
            let end = (sym.end_byte as usize).min(bytes.len());
            let body = String::from_utf8_lossy(&bytes[start..end]);
            let sym_name = state.interner.resolve(&sym.name);
            let key = format!("{file_path}::{sym_name}");
            let is_new = !baseline.ccx_by_sym.contains_key(&key);
            check_body_text_regressions(sym, sym_name, file_path, &body, is_new, regressions);
        }
    }
    let clones = crate::clones::find_clones_in_root(
        &state.ingest,
        &state.interner,
        60,
        None,
        Some(root_path),
    );
    collect_changed_clone_regressions(clones, state, changed_files, regressions);
}

fn filter_regressions_by_scope(
    root: &str,
    regressions: &mut [QualityRegression],
    scope_spec: &str,
) {
    let changed = crate::cli::safety::get_working_tree_changed_files(root);
    let pats: Vec<&str> = scope_spec.split(',').map(str::trim).collect();
    for r in regressions {
        let in_scope = if scope_spec == "diff" {
            changed
                .iter()
                .any(|cf| r.file_path == *cf || r.file_path.ends_with(cf))
        } else {
            pats.iter().any(|&p| r.file_path.contains(p))
        };
        if !in_scope {
            r.is_gating = false;
        }
    }
}

fn regression_matches_ack_filter(r: &QualityRegression, ack_pats: &[&str]) -> bool {
    ack_pats.is_empty()
        || ack_pats.iter().any(|&p| {
            r.kind.contains(p)
                || r.sym_name.contains(p)
                || r.file_path.contains(p)
                || (p == "gating" && r.is_gating)
        })
}

/// Whitespace-insensitive body identity with the symbol's own name replaced by a sentinel byte.
#[must_use]
pub fn content_id(body: &str, own_name: &str) -> u64 {
    // FNV-1a 64-bit offset basis and prime (draft-eastlake-fnv); stable across Rust releases.
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    let normalized = if own_name.is_empty() {
        std::borrow::Cow::Borrowed(body)
    } else {
        std::borrow::Cow::Owned(body.replace(own_name, "\u{1}"))
    };
    normalized
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .fold(OFFSET, |h, b| (h ^ u64::from(b)).wrapping_mul(PRIME))
}

fn regression_content_id(state: &McpState, r: &QualityRegression) -> Option<u64> {
    if matches!(
        r.kind.as_str(),
        "duplication" | "new-clone-of-reused-helper"
    ) {
        return None;
    }
    let file_id = state
        .ingest
        .files
        .iter()
        .position(|f| state.interner.resolve(&f.path) == r.file_path)?;
    let sym =
        state.ingest.symbols.iter().find(|s| {
            s.file_id as usize == file_id && state.interner.resolve(&s.name) == r.sym_name
        })?;
    let bytes = std::fs::read(Path::new(&state.root).join(&r.file_path)).ok()?;
    let start = (sym.sig_start_byte as usize).min(bytes.len());
    let end = (sym.end_byte as usize).min(bytes.len());
    Some(content_id(
        &String::from_utf8_lossy(&bytes[start..end]),
        &r.sym_name,
    ))
}

fn ack_matching_regressions<W: Write>(
    report: &mut QualityDeltaReport,
    ack_pats: &[&str],
    reason: &str,
    cid_for: impl Fn(&QualityRegression) -> Option<u64>,
    ledger: &mut W,
) -> io::Result<()> {
    for r in &mut report.regressions {
        if regression_matches_ack_filter(r, ack_pats) {
            let cid = cid_for(r).map_or_else(String::new, |c| format!("cid={c:016x} "));
            writeln!(
                ledger,
                "ack {} {} {} {cid}now={} was={} p={}:{} {}",
                r.kind, r.sym_name, r.now_val, r.now_val, r.was_val, r.file_path, r.line, reason
            )?;
            r.is_gating = false;
            report.acked_count += 1;
        }
    }
    Ok(())
}

fn stale_replay_for(scope: Option<&str>) -> String {
    match scope {
        Some(s) => format!("--quality-delta --scope={s}"),
        None => "--quality-delta".to_string(),
    }
}

pub fn apply_cli_delta_options(
    state: &McpState,
    report: &mut QualityDeltaReport,
    opts: DeltaCliOptions<'_>,
) -> io::Result<()> {
    let root = state.root.as_str();
    let scope_opt = opts.scope.filter(|s| !s.is_empty());
    if let Some(scope_spec) = scope_opt {
        filter_regressions_by_scope(root, &mut report.regressions, scope_spec);
    }
    if !opts.show_stale {
        report.stale_collapse_replay = Some(stale_replay_for(scope_opt));
    }
    if let Some(reason) = opts.quality_ack {
        let ack_pats = opts.ack_only.map(split_ack_patterns).unwrap_or_default();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(Path::new(root).join(DEFAULT_ACKS_FILE))?;
        let cid_for = |r: &QualityRegression| regression_content_id(state, r);
        ack_matching_regressions(report, &ack_pats, reason, cid_for, &mut file)?;
    }
    report.gating_count = report.regressions.iter().filter(|r| r.is_gating).count();
    report.minor_count = report.regressions.len().saturating_sub(report.gating_count);
    Ok(())
}

fn split_ack_patterns(raw: &str) -> Vec<&str> {
    raw.split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaBasis {
    PinnedOrHead,
    HeadOnly,
}

/// Evaluate quality delta for a workspace root.
pub fn evaluate_quality_delta(root: &str, state: &McpState) -> io::Result<QualityDeltaReport> {
    evaluate_quality_delta_with_basis(root, state, DeltaBasis::PinnedOrHead)
}

pub fn evaluate_quality_delta_with_basis(
    root: &str,
    state: &McpState,
    basis: DeltaBasis,
) -> io::Result<QualityDeltaReport> {
    let at_stamp = crate::git::history::compute_git_at_stamp(root).unwrap_or_default();
    let head_sha = at_stamp
        .strip_suffix("+dirty")
        .unwrap_or(if at_stamp.is_empty() {
            "HEAD"
        } else {
            &at_stamp
        });
    let changed_files = crate::cli::safety::get_working_tree_changed_files(root);
    let mut working = compute_snapshot(&state.ingest, &state.graph, &state.interner, head_sha);
    working.dirty = !changed_files.is_empty();

    let baseline_path = Path::new(root).join(DEFAULT_BASELINE_FILE);
    let (baseline, source_name, head_basis) = if basis == DeltaBasis::PinnedOrHead
        && baseline_path.exists()
    {
        let snap = read_baseline_file(&baseline_path)?;
        (snap, "sidecar", None)
    } else if changed_files.is_empty() {
        (working.clone(), "git-HEAD", Some("identity"))
    } else {
        let snap = load_head_baseline_incremental(root, state, &working, &changed_files, head_sha);
        (snap, "git-HEAD", None)
    };

    let acks_path = Path::new(root).join(DEFAULT_ACKS_FILE);
    let acks = if acks_path.exists() {
        std::fs::read_to_string(&acks_path)
            .ok()
            .map(|c| parse_acks_content(&c))
    } else {
        None
    };

    let mut report =
        compute_quality_delta_with_acks(&working, &baseline, source_name, acks.as_ref());
    report.head_basis = head_basis;
    report.baseline_absorbed = baseline.absorbed;
    if !at_stamp.is_empty() {
        report.at_stamp = at_stamp;
    }
    if !changed_files.is_empty() {
        check_churn_regressions(root, state, &baseline, &mut report.regressions);
        check_source_and_clone_regressions(
            root,
            state,
            &baseline,
            &changed_files,
            &mut report.regressions,
        );
        if let Some(ledger) = &acks {
            let _ = apply_quality_acks(&mut report.regressions, ledger);
        }
    }
    report.gating_count = report.regressions.iter().filter(|r| r.is_gating).count();
    report.minor_count = report.regressions.len().saturating_sub(report.gating_count);
    report.preexisting_worse_count = report
        .regressions
        .iter()
        .filter(|r| !r.is_new_symbol)
        .count();

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::ToXmlString;

    #[test]
    fn test_changed_callable_and_clone_regressions_in_memory() {
        let interner = crate::domain::Interner::new();
        let mut ingest = crate::domain::IngestResult::new();
        ingest.files.push(crate::domain::FileInfo {
            id: 0,
            path: interner.intern("src/a.rs"),
            lang: crate::domain::Lang::Rust,
            line_count: 20,
            byte_size: 200,
        });
        let sym = crate::domain::Symbol::new(
            0,
            interner.intern("dup_fn"),
            interner.intern("src/a.rs::dup_fn"),
            crate::domain::SymKind::Function,
            crate::domain::Lang::Rust,
            0,
            5,
        );
        ingest.symbols.push(sym);
        let changed = vec!["src/a.rs".to_string()];
        assert!(is_changed_callable(&sym, "src/a.rs", &changed));
        assert!(!is_changed_callable(&sym, "src/b.rs", &changed));
        let state = McpState::from_ingest(":memory:", ingest, interner);
        let cg = crate::clones::CloneGroup {
            members: vec![0],
            clone_type: crate::clones::CloneType::Type1,
            token_count: 80,
            similarity: 1.0,
            idiom: crate::clones::CloneIdiom::None,
            tokens_saved: 80,
        };
        let mut regs = Vec::new();
        collect_changed_clone_regressions(vec![cg], &state, &changed, &mut regs);
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].kind, "duplication");
        assert_eq!(regs[0].line, 5);
    }

    #[test]
    fn test_body_text_kinds_ignore_literals_and_read_comments_for_markers() {
        use crate::domain::Lang;
        use crate::metrics::source_mask::mask_source;
        let kinds = |src: &str, lang: Lang| body_text_kinds(&mask_source(src, lang));

        let quoted = "fn f() { let s = \"TODO: catch {} todo!( std::unique(\"; }";
        assert!(kinds(quoted, Lang::Rust).is_empty());
        assert_eq!(
            kinds("fn f() { // TODO: wire it\n}", Lang::Rust),
            ["placeholder"]
        );
        assert_eq!(kinds("fn f() { todo!() }", Lang::Rust), ["placeholder"]);
        assert_eq!(
            kinds(
                "def f():\n    try:\n        g()\n    except:\n        pass\n",
                Lang::Python
            ),
            ["error-masking"]
        );
        assert_eq!(
            kinds("void f() { try { g(); } catch (...) { } }", Lang::Cpp),
            ["error-masking"]
        );
        assert_eq!(
            kinds("void f() { std::unique(v.begin(), v.end()); }", Lang::Cpp),
            ["defect-shape"]
        );
        assert!(
            kinds(
                "void f() { std::sort(a, b); std::unique(a, b); }",
                Lang::Cpp
            )
            .is_empty()
        );
    }

    #[test]
    fn test_stale_rows_collapse_unless_show_stale() {
        let snap = QualitySnapshot::default();
        let mut report = compute_quality_delta_with_acks(&snap, &snap, "git-HEAD", None);
        let stale = |kind: &str, key: &str| StaleAck {
            kind: kind.to_string(),
            key: key.to_string(),
            why: "finding-gone",
            sym: key.to_string(),
            path: "src/a.rs:1".to_string(),
        };
        report.stale_acks = vec![
            stale("complexity", "k1"),
            stale("dead-code:new-symbol", "k2"),
            stale("complexity", "k3"),
        ];

        report.stale_collapse_replay = Some("--quality-delta --scope=src".to_string());
        let collapsed = report.to_xml_string();
        assert!(collapsed.contains("stale=\"3\""));
        assert!(collapsed.contains("stale_by_kind=\"complexity:2,dead-code:1\""));
        assert!(collapsed.contains("stale_next=\"--quality-delta --scope=src --show-stale\""));
        assert!(!collapsed.contains("<sa "));

        report.stale_collapse_replay = None;
        let full = report.to_xml_string();
        assert_eq!(full.matches("<sa ").count(), 3);
        assert!(!full.contains(" stale_by_kind=\""));
    }

    #[test]
    fn test_ack_only_filter_marks_and_records_matching_rows() {
        let snap = QualitySnapshot::default();
        let mut report = compute_quality_delta_with_acks(&snap, &snap, "git-HEAD", None);
        let row = |kind: &str, sym: &str| QualityRegression {
            kind: kind.to_string(),
            sym_name: sym.to_string(),
            file_path: "src/a.rs".to_string(),
            line: 3,
            was_val: 10,
            now_val: 20,
            is_gating: true,
            is_new_symbol: false,
        };
        report.regressions = vec![
            row("complexity", "calc"),
            row("nesting", "walk"),
            row("duplication", "calc"),
        ];
        let mut ledger = Vec::new();
        let cid_for = |r: &QualityRegression| (r.kind != "duplication").then_some(0xabcu64);
        ack_matching_regressions(
            &mut report,
            &split_ack_patterns(" complexity ,duplication"),
            "why",
            cid_for,
            &mut ledger,
        )
        .unwrap();
        let text = String::from_utf8(ledger).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("ack complexity calc 20 cid=0000000000000abc now=20"));
        assert!(lines[0].ends_with("p=src/a.rs:3 why"));
        assert!(lines[1].starts_with("ack duplication calc 20 now=20"));
        assert!(!text.contains("cid=0000000000000000"));
        assert_eq!(report.acked_count, 2);
        assert!(!report.regressions[0].is_gating);
        assert!(report.regressions[1].is_gating);
        assert_eq!(stale_replay_for(Some("src")), "--quality-delta --scope=src");
    }

    #[test]
    fn test_content_id_survives_reindent_and_self_rename_only() {
        let base = content_id("fn calc(x: i32) -> i32 {\n    x + 1\n}", "calc");
        let reindented = content_id("fn calc(x: i32) -> i32 {\n\t\tx + 1\n}", "calc");
        let renamed = content_id("fn total(x: i32) -> i32 {\n    x + 1\n}", "total");
        let changed = content_id("fn calc(x: i32) -> i32 {\n    x + 2\n}", "calc");
        assert_eq!(base, reindented);
        assert_eq!(base, renamed);
        assert_ne!(base, changed);
        assert_ne!(base, 0);
    }

    #[test]
    fn test_quality_delta_complexity_regression() {
        let mut baseline = QualitySnapshot::default();
        baseline
            .ccx_by_sym
            .insert("src/math.rs::calc".to_string(), 10);

        let mut working = QualitySnapshot::default();
        // Now ccx grew from 10 to 20 (> 15 -> gating regression!)
        working
            .ccx_by_sym
            .insert("src/math.rs::calc".to_string(), 20);
        working
            .line_by_sym
            .insert("src/math.rs::calc".to_string(), 42);

        let report = compute_quality_delta(&working, &baseline, "test-baseline");
        assert_eq!(report.regressions.len(), 1);
        assert_eq!(report.gating_count, 1);
        assert!(report.has_gating_failures());

        let reg = &report.regressions[0];
        assert_eq!(reg.kind, "complexity");
        assert_eq!(reg.sym_name, "calc");
        assert_eq!(reg.was_val, 10);
        assert_eq!(reg.now_val, 20);
        assert!(reg.is_gating);
        assert!(!reg.is_new_symbol);
        assert_eq!(reg.line, 42);

        let xml = report.to_xml_string();
        assert!(xml.contains("gating=\"1\""));
        assert!(xml.contains("<r kind=\"complexity\" sym=\"calc\" p=\"src/math.rs:42\""));
    }
    #[test]
    fn test_quality_delta_nesting_and_params_regressions() {
        let mut baseline = QualitySnapshot::default();
        baseline
            .ccx_by_sym
            .insert("src/math.rs::deep".to_string(), 5);
        baseline
            .nest_by_sym
            .insert("src/math.rs::deep".to_string(), 2);
        baseline
            .params_by_sym
            .insert("src/math.rs::deep".to_string(), 2);

        let mut working = QualitySnapshot::default();
        working
            .ccx_by_sym
            .insert("src/math.rs::deep".to_string(), 5);
        working
            .nest_by_sym
            .insert("src/math.rs::deep".to_string(), 7); // now > was and > 6 -> gating
        working
            .params_by_sym
            .insert("src/math.rs::deep".to_string(), 6); // now > was and > 5 -> gating

        let report = compute_quality_delta(&working, &baseline, "test-baseline");
        assert_eq!(report.regressions.len(), 2);
        assert_eq!(report.gating_count, 2);
        assert!(report.has_gating_failures());

        let kinds: Vec<&str> = report.regressions.iter().map(|r| r.kind.as_str()).collect();
        assert!(kinds.contains(&"nesting"));
        assert!(kinds.contains(&"params"));
    }

    #[test]
    fn test_quality_delta_acks_suppression() {
        let mut baseline = QualitySnapshot::default();
        baseline
            .ccx_by_sym
            .insert("src/math.rs::deep".to_string(), 5);
        baseline
            .nest_by_sym
            .insert("src/math.rs::deep".to_string(), 2);

        let mut working = QualitySnapshot::default();
        working
            .ccx_by_sym
            .insert("src/math.rs::deep".to_string(), 5);
        working
            .nest_by_sym
            .insert("src/math.rs::deep".to_string(), 7);

        let acks_content =
            "ack nesting 70a00a1f5e885d55 10 p=src/math.rs:1 deliberate deep nesting\n";
        let acks = parse_acks_content(acks_content);

        let report =
            compute_quality_delta_with_acks(&working, &baseline, "test-baseline", Some(&acks));
        assert_eq!(report.regressions.len(), 1);
        assert_eq!(report.gating_count, 0); // suppressed by ack!
        assert!(!report.has_gating_failures());
    }
    #[test]
    fn test_symbol_matches_and_changed() {
        use crate::domain::{FileInfo, IngestResult, Interner, Lang, SymKind, Symbol};

        let mut baseline = QualitySnapshot::default();
        baseline
            .ccx_by_sym
            .insert("src/math.rs::calc".to_string(), 10);
        baseline
            .loc_by_sym
            .insert("src/math.rs::calc".to_string(), 20);

        let interner = Interner::new();
        let sym_name = interner.intern("calc");
        let path = interner.intern("src/math.rs");
        let mut sym = Symbol::new(
            0,
            sym_name,
            interner.intern("src/math.rs::calc"),
            SymKind::Function,
            Lang::Rust,
            0,
            10,
        );
        sym.loc = 30; // changed from 20!
        sym.ccx = 10;
        let ingest = IngestResult {
            files: vec![FileInfo {
                id: 0,
                path,
                lang: Lang::Rust,
                byte_size: 100,
                line_count: 100,
            }],
            symbols: vec![sym],
            references: Vec::new(),
        };
        let changed =
            symbol_matches_and_changed(&sym, "src/math.rs", &ingest, &interner, &baseline);
        assert!(changed.is_some());
    }
}
