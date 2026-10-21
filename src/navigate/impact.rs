use super::ImpactNode;
use crate::domain::{CorpusView, IngestResult, Interner, NodeId, RefRole};
use crate::resolve::{RefOutcome, ResolutionIndex, ResolutionLookup};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactDef<'a> {
    pub kind: &'static str,
    pub scope: Option<&'a str>,
    pub path: &'a str,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclinedCall<'a> {
    pub enclosing: &'a str,
    pub path: &'a str,
    pub line: u32,
    pub cands: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReachFile<'a> {
    pub path: &'a str,
    pub syms: usize,
    pub hops: u32,
}

#[must_use]
pub fn impact_defs<'a>(
    ids: &[NodeId],
    ingest: &'a IngestResult,
    interner: &'a Interner,
) -> Vec<ImpactDef<'a>> {
    let view = (ingest, interner);
    let mut defs: Vec<ImpactDef<'a>> = ids
        .iter()
        .filter_map(|&id| ingest.sym(id))
        .map(|s| ImpactDef {
            kind: s.kind.tag(),
            scope: view.sym_scope(s),
            path: view.file_path(s.file_id),
            line: s.line,
        })
        .collect();
    defs.sort_by(|a, b| (a.path, a.line).cmp(&(b.path, b.line)));
    defs
}

/// Declined call sites naming a seed or reached symbol, interleaved file-first
/// (each file's first site in path order, then each file's second, ...).
#[must_use]
pub fn declined_calls<'a>(
    seeds: &[NodeId],
    nodes: &[ImpactNode],
    ingest: &'a IngestResult,
    interner: &'a Interner,
) -> Vec<DeclinedCall<'a>> {
    let names: HashSet<lasso::Spur> = seeds
        .iter()
        .copied()
        .chain(nodes.iter().map(|n| n.id))
        .filter_map(|id| ingest.sym(id).map(|s| s.name))
        .collect();
    let index = ResolutionIndex::build(ingest, interner);
    let lookup = index.context(ingest, interner);
    let view = (ingest, interner);
    let mut sites: Vec<DeclinedCall<'a>> = ingest
        .references
        .iter()
        .filter(|r| matches!(r.role, RefRole::Call | RefRole::Macro))
        .filter(|r| names.contains(&r.callee_name))
        .filter(|r| lookup.classify_ref(r) == RefOutcome::Declined)
        .filter_map(|r| {
            Some(DeclinedCall {
                enclosing: view.sym_name(ingest.sym(r.from_symbol)?),
                path: view.file_path(r.file_id),
                line: r.line,
                cands: lookup.by_name().get(&r.callee_name).map_or(0, Vec::len),
            })
        })
        .collect();
    sites.sort_by(|a, b| (a.path, a.line).cmp(&(b.path, b.line)));
    let mut nth_in_file = Vec::with_capacity(sites.len());
    for (i, site) in sites.iter().enumerate() {
        let same_file = i > 0 && sites[i - 1].path == site.path;
        let nth = if same_file { nth_in_file[i - 1] + 1 } else { 0 };
        nth_in_file.push(nth);
    }
    let mut ranked: Vec<(usize, DeclinedCall<'a>)> = nth_in_file.into_iter().zip(sites).collect();
    ranked.sort_by_key(|(nth, _)| *nth);
    ranked.into_iter().map(|(_, site)| site).collect()
}

#[must_use]
pub fn reach_files(nodes: &[ImpactNode]) -> Vec<ReachFile<'_>> {
    let mut by_path: BTreeMap<&str, ReachFile<'_>> = BTreeMap::new();
    for n in nodes {
        let entry = by_path.entry(&n.path).or_insert(ReachFile {
            path: &n.path,
            syms: 0,
            hops: n.hops,
        });
        entry.syms += 1;
        entry.hops = entry.hops.min(n.hops);
    }
    let mut files: Vec<ReachFile<'_>> = by_path.into_values().collect();
    files.sort_by(|a, b| b.syms.cmp(&a.syms).then_with(|| a.path.cmp(b.path)));
    files
}
