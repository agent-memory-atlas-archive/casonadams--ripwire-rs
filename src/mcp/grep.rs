use crate::domain::{IngestResult, Interner, NodeId, RefRole, SymKind, Symbol};
use crate::graph::Graph;
use crate::languages::source::{SourceRegion, source_regions};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;

/// String-literal hits ride along with code hits only while code hits stay below this count.
const STRING_LIFT_MAX_CODE_HITS: usize = 8;
const READS_AT_SHOWN: usize = 3;
const UNINDEXED_CANDIDATE_CAP: usize = 500;

#[derive(Clone, Copy)]
pub struct GrepRequest<'a> {
    pub root: &'a str,
    pub pattern: &'a str,
    pub verb: &'a str,
    pub path_filter: Option<&'a str>,
    pub any_tier: bool,
    pub before: usize,
    pub after: usize,
    pub and_terms: &'a [String],
    pub not_terms: &'a [String],
    pub file_scope: bool,
    pub graph: Option<&'a Graph>,
}

impl<'a> GrepRequest<'a> {
    #[must_use]
    pub const fn new(root: &'a str, pattern: &'a str) -> Self {
        Self {
            root,
            pattern,
            verb: "grep",
            path_filter: None,
            any_tier: false,
            before: 0,
            after: 0,
            and_terms: &[],
            not_terms: &[],
            file_scope: false,
            graph: None,
        }
    }

    fn file_passes(&self, text: &str) -> bool {
        !self.file_scope || self.terms_pass(text)
    }

    fn line_passes(&self, line: &str) -> bool {
        self.file_scope || self.terms_pass(line)
    }

    fn terms_pass(&self, text: &str) -> bool {
        self.and_terms.iter().all(|t| text.contains(t.as_str()))
            && !self.not_terms.iter().any(|t| text.contains(t.as_str()))
    }

    fn path_passes(&self, path: &str) -> bool {
        self.path_filter.is_none_or(|pf| path.contains(pf))
    }
}

struct LineHit {
    line_idx: usize,
    offset: usize,
    region: SourceRegion,
}

struct HitFile<'a> {
    path: String,
    prose: bool,
    text: String,
    lines: Vec<Range<usize>>,
    hits: Vec<LineHit>,
    syms: Vec<&'a Symbol>,
}

fn line_spans(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split_inclusive('\n').scan(0usize, |start, raw| {
        let begin = *start;
        *start += raw.len();
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        Some((begin, line.strip_suffix('\r').unwrap_or(line)))
    })
}

fn scan_text(
    text: &str,
    regions: Option<&[SourceRegion]>,
    req: &GrepRequest<'_>,
) -> (Vec<Range<usize>>, Vec<LineHit>) {
    let mut hits = Vec::new();
    if req.pattern.is_empty() || !req.file_passes(text) {
        return (Vec::new(), hits);
    }
    let mut lines = Vec::new();
    for (line_idx, (begin, line)) in line_spans(text).enumerate() {
        lines.push(begin..begin + line.len());
        if !line.contains(req.pattern) || !req.line_passes(line) {
            continue;
        }
        hits.extend(line.match_indices(req.pattern).map(|(col, _)| LineHit {
            line_idx,
            offset: begin + col,
            region: regions.map_or(SourceRegion::Code, |r| r[begin + col]),
        }));
    }
    (lines, hits)
}

fn scan_indexed<'a>(
    ingest: &'a IngestResult,
    interner: &Interner,
    req: &GrepRequest<'_>,
) -> Vec<HitFile<'a>> {
    let root_path = std::path::Path::new(req.root);
    let mut syms_by_file: HashMap<u32, Vec<&Symbol>> = HashMap::new();
    for s in ingest
        .symbols
        .iter()
        .filter(|s| s.kind != SymKind::ModuleScope)
    {
        syms_by_file.entry(s.file_id).or_default().push(s);
    }
    let mut files = Vec::new();
    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        if !req.path_passes(path) {
            continue;
        }
        let Ok(bytes) = crate::domain::read_rel_bytes(root_path, path) else {
            continue;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        if !text.contains(req.pattern) || !req.file_passes(&text) {
            continue;
        }
        let prose = file.lang.semantics().is_prose_document();
        let regions = (!prose).then(|| source_regions(&text, file.lang, path));
        let (lines, hits) = scan_text(&text, regions.as_deref(), req);
        if !hits.is_empty() {
            files.push(HitFile {
                path: path.to_string(),
                prose,
                text,
                lines,
                hits,
                syms: syms_by_file.remove(&file.id).unwrap_or_default(),
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TierCounts {
    pub code: usize,
    pub comment: usize,
    pub literal: usize,
}

impl TierCounts {
    fn of(files: &[HitFile<'_>]) -> Self {
        let mut c = Self::default();
        for h in files.iter().flat_map(|f| &f.hits) {
            match h.region {
                SourceRegion::Code => c.code += 1,
                SourceRegion::Comment => c.comment += 1,
                SourceRegion::Literal => c.literal += 1,
            }
        }
        c
    }
}

struct TierPolicy {
    comment: bool,
    literal: bool,
}

impl TierPolicy {
    fn select(c: &TierCounts, any_tier: bool) -> Self {
        let lift_all = any_tier || c.code == 0;
        Self {
            comment: lift_all,
            literal: lift_all || c.code < STRING_LIFT_MAX_CODE_HITS,
        }
    }

    const fn serves(&self, region: SourceRegion) -> bool {
        match region {
            SourceRegion::Code => true,
            SourceRegion::Comment => self.comment,
            SourceRegion::Literal => self.literal,
        }
    }
}

fn tier_label(c: &TierCounts) -> Option<&'static str> {
    match (c.code > 0, c.comment > 0, c.literal > 0) {
        (true, _, true) => Some("code+string"),
        (false, true, true) => Some("comment+string"),
        (false, true, false) => Some("comment"),
        (false, false, true) => Some("string"),
        _ => None,
    }
}

#[must_use]
pub fn tier_attrs(c: TierCounts, hit_files: usize, any_tier: bool) -> Map<String, Value> {
    let mut obj = Map::new();
    if any_tier || c.comment + c.literal == 0 {
        return obj;
    }
    let policy = TierPolicy::select(&c, false);
    if !policy.comment && c.comment > 0 {
        obj.insert("suppressed_comment".into(), json!(c.comment));
    }
    if !policy.literal && c.literal > 0 {
        obj.insert("suppressed_string".into(), json!(c.literal));
    }
    let served = TierCounts {
        code: c.code,
        comment: if policy.comment { c.comment } else { 0 },
        literal: if policy.literal { c.literal } else { 0 },
    };
    if let Some(label) = tier_label(&served) {
        obj.insert("tier".into(), json!(label));
        if label == "code+string" {
            obj.insert("string_hits".into(), json!(served.literal));
        }
    }
    obj.insert("tier_parsed".into(), json!(hit_files));
    obj.insert("tier_unclassified".into(), json!(0));
    obj
}

fn enclosing_symbol<'a>(syms: &[&'a Symbol], offset: usize) -> Option<&'a Symbol> {
    syms.iter()
        .filter(|s| (s.sig_start_byte as usize..s.end_byte as usize).contains(&offset))
        .max_by_key(|s| (s.sig_start_byte, s.line))
        .copied()
}

fn sym_label(sym: &Symbol, interner: &Interner) -> String {
    let name = interner.resolve(&sym.name);
    sym.scope.map_or_else(
        || name.to_string(),
        |sc| format!("{}::{name}", interner.resolve(&sc)),
    )
}

fn context_rows<'a>(text: &'a str, lines: &[Range<usize>]) -> Vec<&'a str> {
    lines.iter().map(|range| &text[range.clone()]).collect()
}

fn hit_json(file: &HitFile<'_>, h: &LineHit, req: &GrepRequest<'_>, interner: &Interner) -> Value {
    let line_num = h.line_idx as u32 + 1;
    let mut obj = json!({
        "path": file.path,
        "line": line_num,
        "text": &file.text[file.lines[h.line_idx].clone()],
    });
    if let Some(sym) = enclosing_symbol(&file.syms, h.offset) {
        obj["enclosing"] = json!(sym_label(sym, interner));
        obj["enc_id"] = json!(sym.id);
    }
    if req.before > 0 {
        let start = h.line_idx.saturating_sub(req.before);
        obj["before"] = json!(context_rows(&file.text, &file.lines[start..h.line_idx]));
    }
    if req.after > 0 {
        let end = (h.line_idx + 1 + req.after).min(file.lines.len());
        obj["after"] = json!(context_rows(&file.text, &file.lines[h.line_idx + 1..end]));
    }
    obj
}

fn served_hits(files: &[HitFile<'_>], req: &GrepRequest<'_>, interner: &Interner) -> Vec<Value> {
    let policy = TierPolicy::select(&TierCounts::of(files), req.any_tier);
    let mut grouped: Vec<(bool, Vec<Value>)> = files
        .iter()
        .map(|f| {
            let rows: Vec<&LineHit> = f.hits.iter().filter(|h| policy.serves(h.region)).collect();
            let code_file = !f.prose && rows.iter().any(|h| h.region == SourceRegion::Code);
            let rows = rows.iter().map(|h| hit_json(f, h, req, interner)).collect();
            (!code_file, rows)
        })
        .collect();
    grouped.sort_by_key(|(rest, _)| *rest);
    grouped.into_iter().flat_map(|(_, rows)| rows).collect()
}

fn resolved_callers(graph: Option<&Graph>, defs: &[&Symbol]) -> Option<HashSet<NodeId>> {
    graph.map(|g| {
        defs.iter()
            .flat_map(|d| g.callers(d.id))
            .map(|(c, _)| c)
            .collect()
    })
}

fn read_sites(ingest: &IngestResult, interner: &Interner, name: lasso::Spur) -> Vec<String> {
    let mut sites: Vec<(&str, u32)> = ingest
        .references
        .iter()
        .filter(|r| r.role == RefRole::Read && r.callee_name == name)
        .map(|r| (ingest.file_path(r.file_id, interner), r.line))
        .collect();
    sites.sort_unstable();
    sites.dedup();
    sites.into_iter().map(|(p, l)| format!("{p}:{l}")).collect()
}

struct EncCtx<'a> {
    ingest: &'a IngestResult,
    interner: &'a Interner,
    graph: Option<&'a Graph>,
}

fn enc_json(ctx: &EncCtx<'_>, defs: &[&Symbol]) -> Value {
    let callers = resolved_callers(ctx.graph, defs);
    let spur = defs[0].name;
    let unresolved_call = callers.as_ref().is_some_and(|set| {
        ctx.ingest.references.iter().any(|r| {
            r.role == RefRole::Call && r.callee_name == spur && !set.contains(&r.from_symbol)
        })
    });
    let reads = read_sites(ctx.ingest, ctx.interner, spur);
    let read_kind = !reads.is_empty()
        || defs
            .iter()
            .any(|d| d.kind == SymKind::Var || d.kind.is_type_container());
    let reads_indexed = defs.iter().any(|d| d.lang.semantics().indexes_reads());
    let plain = ctx.interner.resolve(&spur);
    let mut obj = json!({
        "floor": read_kind || unresolved_call,
        "floor_next": format!("--{}={plain}", if reads_indexed { "uses" } else { "grep" }),
        "cx": defs.iter().map(|d| d.cx).max().unwrap_or(0),
        "reads": reads.len(),
        "reads_at": &reads[..reads.len().min(READS_AT_SHOWN)],
    });
    if let Some(set) = callers {
        obj["callers"] = json!(set.len());
    }
    obj
}

pub fn enclosing_metadata(
    ingest: &IngestResult,
    interner: &Interner,
    graph: Option<&Graph>,
    hits: &[Value],
) -> Map<String, Value> {
    let ctx = EncCtx {
        ingest,
        interner,
        graph,
    };
    let names: HashSet<&str> = hits
        .iter()
        .filter_map(|h| h["enclosing"].as_str())
        .collect();
    let mut defs: BTreeMap<String, Vec<&Symbol>> = BTreeMap::new();
    for s in ctx
        .ingest
        .symbols
        .iter()
        .filter(|s| s.kind != SymKind::ModuleScope)
    {
        let label = sym_label(s, ctx.interner);
        if names.contains(label.as_str()) {
            defs.entry(label).or_default().push(s);
        }
    }
    defs.into_iter()
        .map(|(n, d)| {
            let info = enc_json(&ctx, &d);
            (n, info)
        })
        .collect()
}

fn unindexed_text_hits(root: &std::path::Path, rel: &str, req: &GrepRequest<'_>) -> Vec<Value> {
    let Ok(bytes) = crate::domain::read_rel_bytes(root, rel) else {
        return Vec::new();
    };
    if bytes.len() as u64 > crate::crawler::DEFAULT_MAX_FILE_SIZE
        || crate::crawler::is_binary_buffer(&bytes)
    {
        return Vec::new();
    }
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Vec::new();
    };
    let (lines, line_hits) = scan_text(text, None, req);
    line_hits
        .iter()
        .map(|h| json!({"path": rel, "line": h.line_idx + 1, "text": &text[lines[h.line_idx].clone()]}))
        .collect()
}

fn scan_unindexed(ingest: &IngestResult, interner: &Interner, req: &GrepRequest<'_>) -> Value {
    let walk = crate::crawler::walk_corpus(req.root, &crate::crawler::CrawlerOptions::default());
    let indexed: HashSet<&str> = ingest
        .files
        .iter()
        .map(|f| interner.resolve(&f.path))
        .collect();
    let candidates: Vec<&String> = walk
        .rel_paths
        .iter()
        .filter(|rel| {
            std::path::Path::new(rel).extension().is_some()
                && !indexed.contains(rel.as_str())
                && req.path_passes(rel)
        })
        .collect();
    let scanned = &candidates[..candidates.len().min(UNINDEXED_CANDIDATE_CAP)];
    let root = std::path::Path::new(req.root);
    let hits: Vec<Value> = scanned
        .iter()
        .flat_map(|rel| unindexed_text_hits(root, rel, req))
        .collect();
    let mut obj = json!({
        "corpus_pruned_dirs": walk.pruned_dirs,
        "unindexed": hits,
        "unindexed_files_scanned": scanned.len(),
    });
    if candidates.len() > scanned.len() {
        obj["unindexed_candidates_capped"] = json!(true);
    }
    obj
}

#[must_use]
pub fn call_grep(ingest: &IngestResult, interner: &Interner, req: &GrepRequest<'_>) -> Value {
    let files = scan_indexed(ingest, interner, req);
    let hits = served_hits(&files, req, interner);
    let mut obj = Map::new();
    obj.insert("pattern".into(), json!(req.pattern));
    obj.insert("root".into(), json!(req.root));
    obj.insert("verb".into(), json!(req.verb));
    obj.insert("literal".into(), json!(req.verb == "grep"));
    obj.insert("context".into(), json!(req.before + req.after > 0));
    obj.extend(tier_attrs(
        TierCounts::of(&files),
        files.len(),
        req.any_tier,
    ));
    let enc = enclosing_metadata(ingest, interner, req.graph, &hits);
    obj.insert("enc".into(), Value::Object(enc));
    obj.insert("hits".into(), Value::Array(hits));
    if let Value::Object(extra) = scan_unindexed(ingest, interner, req) {
        obj.extend(extra);
    }
    Value::Object(obj)
}
