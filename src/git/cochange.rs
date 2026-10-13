use crate::domain::{IngestResult, Interner, Lang, RefRole};
use crate::git::history::CommitInfo;
use crate::graph::Graph;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Configuration options for the co-change miner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CochangeOptions {
    pub min_support: u32,
    pub sub_windows: usize,
    pub max_commit_files: usize,
}

impl Default for CochangeOptions {
    fn default() -> Self {
        Self {
            min_support: 3,
            sub_windows: 3,
            max_commit_files: 100,
        }
    }
}

/// A co-change partner of a specific probed file.
#[derive(Debug, Clone, PartialEq)]
pub struct CoPartner {
    pub file_path: String,
    pub together: u32,
    pub deg: f64,
    pub deg_rev: f64,
    pub recur: u32,
    pub surprising: bool,
    pub dep_capable: bool,
}

/// A repo-wide co-change pair between two files.
#[derive(Debug, Clone, PartialEq)]
pub struct CoPair {
    pub file_a: String,
    pub file_b: String,
    pub together: u32,
    pub deg: f64,
    pub recur: u32,
    pub surprising: bool,
    pub dep_capable: bool,
}

/// Fast lookup for static dependency coupling between files.
#[derive(Debug, Default)]
pub struct StaticCouplingMap {
    file_to_id: HashMap<String, u32>,
    transitive_closure: Vec<HashSet<u32>>,
    referenced_basenames: Vec<HashSet<String>>,
    file_basenames: Vec<String>,
}

impl StaticCouplingMap {
    /// Build static coupling map from an IngestResult and call Graph.
    #[must_use]
    pub fn build(ingest: &IngestResult, interner: &Interner, graph: Option<&Graph>) -> Self {
        let (file_to_id, file_basenames, referenced_basenames) =
            extract_file_metadata(ingest, interner);
        let direct_adj = build_direct_file_adj(ingest, graph);
        let transitive_closure = compute_transitive_closure(&direct_adj);

        Self {
            file_to_id,
            transitive_closure,
            referenced_basenames,
            file_basenames,
        }
    }

    /// Check if two file paths are statically coupled.
    #[must_use]
    pub fn is_coupled(&self, path_a: &str, path_b: &str) -> bool {
        let id_a = match self.file_to_id.get(path_a) {
            Some(&id) => id as usize,
            None => return false,
        };
        let id_b = match self.file_to_id.get(path_b) {
            Some(&id) => id as usize,
            None => return false,
        };
        if id_a == id_b {
            return true;
        }

        if self.names_basename(id_a, id_b) || self.names_basename(id_b, id_a) {
            return true;
        }

        let a_reaches_b = self
            .transitive_closure
            .get(id_a)
            .is_some_and(|c| c.contains(&(id_b as u32)));
        let b_reaches_a = self
            .transitive_closure
            .get(id_b)
            .is_some_and(|c| c.contains(&(id_a as u32)));

        a_reaches_b || b_reaches_a
    }

    fn names_basename(&self, source_id: usize, target_id: usize) -> bool {
        let target_base = match self.file_basenames.get(target_id) {
            Some(b) if !b.is_empty() => b,
            _ => return false,
        };
        if let Some(refs) = self.referenced_basenames.get(source_id) {
            if refs.contains(target_base) {
                return true;
            }
            let stem = Path::new(target_base)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if !stem.is_empty() && refs.contains(stem) {
                return true;
            }
        }
        false
    }
}

type FileMetadata = (HashMap<String, u32>, Vec<String>, Vec<HashSet<String>>);

fn extract_file_metadata(ingest: &IngestResult, interner: &Interner) -> FileMetadata {
    let n_files = ingest.files.len();
    let mut file_to_id = HashMap::with_capacity(n_files);
    let mut file_basenames = Vec::with_capacity(n_files);
    let mut referenced_basenames = vec![HashSet::new(); n_files];

    for (idx, f) in ingest.files.iter().enumerate() {
        let path_str = interner.resolve(&f.path).to_string();
        let base = Path::new(&path_str)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        file_to_id.insert(path_str, idx as u32);
        file_basenames.push(base);
    }

    for r in &ingest.references {
        if r.role == RefRole::Import || r.role == RefRole::Call {
            let name = interner.resolve(&r.callee_name).to_string();
            if let Some(set) = referenced_basenames.get_mut(r.file_id as usize) {
                set.insert(name);
            }
        }
    }

    (file_to_id, file_basenames, referenced_basenames)
}

fn build_direct_file_adj(ingest: &IngestResult, graph: Option<&Graph>) -> Vec<HashSet<u32>> {
    let n_files = ingest.files.len();
    let mut direct_adj = vec![HashSet::new(); n_files];
    let Some(g) = graph else {
        return direct_adj;
    };

    for from_node in 0..g.node_count() {
        let from_fid = ingest.symbols[from_node].file_id as usize;
        for (to_node, _) in g.callees(from_node as u32) {
            let to_fid = ingest.symbols[to_node as usize].file_id as usize;
            if from_fid != to_fid && from_fid < n_files && to_fid < n_files {
                direct_adj[from_fid].insert(to_fid as u32);
            }
        }
    }
    direct_adj
}

fn forward_reachable(start: u32, direct_adj: &[HashSet<u32>]) -> HashSet<u32> {
    let mut visited = HashSet::new();
    let mut queue = vec![start];
    visited.insert(start);
    while let Some(curr) = queue.pop() {
        if let Some(neighbors) = direct_adj.get(curr as usize) {
            for &next in neighbors {
                if visited.insert(next) {
                    queue.push(next);
                }
            }
        }
    }
    visited
}

fn compute_transitive_closure(direct_adj: &[HashSet<u32>]) -> Vec<HashSet<u32>> {
    (0..direct_adj.len())
        .map(|i| forward_reachable(i as u32, direct_adj))
        .collect()
}
fn is_file_path(p: &str) -> bool {
    Path::new(p).extension().is_some()
}

/// Determine whether two file paths are capable of depending on each other.
#[must_use]
pub fn check_pair_dependency_capability(
    path_a: &str,
    path_b: &str,
    ingest: Option<&IngestResult>,
    interner: Option<&Interner>,
) -> bool {
    let ext_a = Path::new(path_a)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let ext_b = Path::new(path_b)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let mut lang_a = Lang::from_extension(ext_a);
    let mut lang_b = Lang::from_extension(ext_b);

    if (lang_a == Lang::Unknown || lang_b == Lang::Unknown)
        && let (Some(ing), Some(itr)) = (ingest, interner)
    {
        for f in &ing.files {
            let p = itr.resolve(&f.path);
            if lang_a == Lang::Unknown && p == path_a {
                lang_a = f.lang;
            }
            if lang_b == Lang::Unknown && p == path_b {
                lang_b = f.lang;
            }
        }
    }

    lang_a.is_pair_dependency_capable(lang_b)
}

struct FileCommitStats {
    together: u32,
    recur_mask: u32,
}

struct FileCochangeTally<'a> {
    probed_commits: u32,
    other_total: HashMap<&'a str, u32>,
    partners: HashMap<&'a str, FileCommitStats>,
}

fn tally_file_cochange<'a>(
    probed_path: &str,
    commits: &[&'a CommitInfo],
    sub_windows: usize,
) -> FileCochangeTally<'a> {
    let mut probed_commits = 0u32;
    let mut other_total: HashMap<&'a str, u32> = HashMap::new();
    let mut partners: HashMap<&'a str, FileCommitStats> = HashMap::new();

    for (idx, commit) in commits.iter().enumerate() {
        let has_probed = commit.changed_files.iter().any(|p| p == probed_path);
        if has_probed {
            probed_commits += 1;
        }

        let bit = 1u32 << ((idx * sub_windows) / commits.len());
        for file in &commit.changed_files {
            if file != probed_path {
                *other_total.entry(file.as_str()).or_insert(0) += 1;
                if has_probed {
                    let entry = partners.entry(file.as_str()).or_insert(FileCommitStats {
                        together: 0,
                        recur_mask: 0,
                    });
                    entry.together += 1;
                    entry.recur_mask |= bit;
                }
            }
        }
    }
    FileCochangeTally {
        probed_commits,
        other_total,
        partners,
    }
}

/// Mine co-change partners for a single probed file.
#[must_use]
pub fn mine_cochange_for_file<'a, I>(
    probed_path: &str,
    commits: I,
    coupling: Option<&StaticCouplingMap>,
    ingest: Option<&IngestResult>,
    interner: Option<&Interner>,
    opts: &CochangeOptions,
) -> Vec<CoPartner>
where
    I: IntoIterator<Item = &'a CommitInfo>,
{
    let commit_list: Vec<&'a CommitInfo> = commits
        .into_iter()
        .filter(|c| c.changed_files.len() <= opts.max_commit_files)
        .collect();

    if commit_list.is_empty() {
        return Vec::new();
    }

    let sub_windows = opts.sub_windows.min(commit_list.len()).max(1);
    let tally = tally_file_cochange(probed_path, &commit_list, sub_windows);

    if tally.probed_commits == 0 {
        return Vec::new();
    }

    let mut partners = Vec::new();
    for (partner_path, stats) in tally.partners {
        if !is_file_path(partner_path) {
            continue;
        }
        if stats.together >= opts.min_support {
            let total = tally
                .other_total
                .get(partner_path)
                .copied()
                .unwrap_or(stats.together);
            let deg = (stats.together as f64) / (tally.probed_commits as f64);
            let deg_rev = (stats.together as f64) / (total as f64);
            let recur = stats.recur_mask.count_ones();
            let dep_capable =
                check_pair_dependency_capability(probed_path, partner_path, ingest, interner);
            let is_coupled = coupling.is_some_and(|c| c.is_coupled(probed_path, partner_path));
            let surprising = dep_capable && !is_coupled;

            partners.push(CoPartner {
                file_path: partner_path.to_string(),
                together: stats.together,
                deg,
                deg_rev,
                recur,
                surprising,
                dep_capable,
            });
        }
    }

    partners.sort_by(|a, b| {
        b.deg
            .partial_cmp(&a.deg)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.file_path.cmp(&b.file_path))
    });

    partners
}

struct RepoPairsTally<'a> {
    file_counts: HashMap<&'a str, u32>,
    pairs: HashMap<(&'a str, &'a str), FileCommitStats>,
}

fn tally_repo_pairs<'a>(commits: &[&'a CommitInfo], sub_windows: usize) -> RepoPairsTally<'a> {
    let mut file_counts: HashMap<&'a str, u32> = HashMap::new();
    let mut pairs: HashMap<(&'a str, &'a str), FileCommitStats> = HashMap::new();

    for (idx, commit) in commits.iter().enumerate() {
        let bit = 1u32 << ((idx * sub_windows) / commits.len());
        for f in &commit.changed_files {
            *file_counts.entry(f.as_str()).or_insert(0) += 1;
        }

        let n = commit.changed_files.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let fa = commit.changed_files[i].as_str();
                let fb = commit.changed_files[j].as_str();
                let key = if fa < fb { (fa, fb) } else { (fb, fa) };
                let entry = pairs.entry(key).or_insert(FileCommitStats {
                    together: 0,
                    recur_mask: 0,
                });
                entry.together += 1;
                entry.recur_mask |= bit;
            }
        }
    }
    RepoPairsTally { file_counts, pairs }
}

/// Mine repository-wide co-change pairs across all files.
#[must_use]
pub fn mine_repo_cochange_pairs<'a, I>(
    commits: I,
    coupling: Option<&StaticCouplingMap>,
    ingest: Option<&IngestResult>,
    interner: Option<&Interner>,
    opts: &CochangeOptions,
) -> Vec<CoPair>
where
    I: IntoIterator<Item = &'a CommitInfo>,
{
    let commit_list: Vec<&'a CommitInfo> = commits
        .into_iter()
        .filter(|c| c.changed_files.len() <= opts.max_commit_files)
        .collect();

    if commit_list.is_empty() {
        return Vec::new();
    }

    let sub_windows = opts.sub_windows.min(commit_list.len()).max(1);
    let tally = tally_repo_pairs(&commit_list, sub_windows);

    let mut pairs = Vec::new();
    for ((fa, fb), stats) in tally.pairs {
        if !is_file_path(fa) || !is_file_path(fb) {
            continue;
        }
        if stats.together >= opts.min_support {
            let total_a = tally.file_counts.get(fa).copied().unwrap_or(stats.together);
            let total_b = tally.file_counts.get(fb).copied().unwrap_or(stats.together);
            let deg_a = (stats.together as f64) / (total_a as f64);
            let deg_b = (stats.together as f64) / (total_b as f64);
            let deg = deg_a.max(deg_b);
            let recur = stats.recur_mask.count_ones();
            let dep_capable = check_pair_dependency_capability(fa, fb, ingest, interner);
            let is_coupled = coupling.is_some_and(|c| c.is_coupled(fa, fb));
            let surprising = dep_capable && !is_coupled;

            pairs.push(CoPair {
                file_a: fa.to_string(),
                file_b: fb.to_string(),
                together: stats.together,
                deg,
                recur,
                surprising,
                dep_capable,
            });
        }
    }

    pairs.sort_by(|a, b| {
        b.together
            .cmp(&a.together)
            .then_with(|| {
                b.deg
                    .partial_cmp(&a.deg)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.file_a.cmp(&b.file_a))
            .then_with(|| a.file_b.cmp(&b.file_b))
    });

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cochange_mining_and_surprise() {
        let commits = vec![
            CommitInfo {
                id: "c1".to_string(),
                timestamp: 100,
                author: "alice@example.com".to_string(),
                changed_files: vec![
                    "src/ui.rs".to_string(),
                    "src/db.rs".to_string(),
                    "docs/readme.md".to_string(),
                ],
            },
            CommitInfo {
                id: "c2".to_string(),
                timestamp: 200,
                author: "alice@example.com".to_string(),
                changed_files: vec![
                    "src/ui.rs".to_string(),
                    "src/db.rs".to_string(),
                    "docs/readme.md".to_string(),
                ],
            },
            CommitInfo {
                id: "c3".to_string(),
                timestamp: 300,
                author: "bob@example.com".to_string(),
                changed_files: vec![
                    "src/ui.rs".to_string(),
                    "src/db.rs".to_string(),
                    "docs/readme.md".to_string(),
                ],
            },
        ];

        let opts = CochangeOptions {
            min_support: 3,
            sub_windows: 3,
            max_commit_files: 100,
        };

        // Without static coupling, src/ui.rs <-> src/db.rs is surprising
        let partners = mine_cochange_for_file("src/ui.rs", &commits, None, None, None, &opts);
        assert_eq!(partners.len(), 2);

        let db_partner = partners
            .iter()
            .find(|p| p.file_path == "src/db.rs")
            .unwrap();
        assert_eq!(db_partner.together, 3);
        assert_eq!(db_partner.deg, 1.0);
        assert_eq!(db_partner.recur, 3);
        assert!(db_partner.dep_capable);
        assert!(db_partner.surprising);

        let doc_partner = partners
            .iter()
            .find(|p| p.file_path == "docs/readme.md")
            .unwrap();
        assert_eq!(doc_partner.together, 3);
        assert!(!doc_partner.dep_capable);
        assert!(!doc_partner.surprising);

        let pairs = mine_repo_cochange_pairs(&commits, None, None, None, &opts);
        assert_eq!(pairs.len(), 3);
        let ui_db_pair = pairs
            .iter()
            .find(|p| p.file_a == "src/db.rs" && p.file_b == "src/ui.rs")
            .unwrap();
        assert_eq!(ui_db_pair.together, 3);
        assert!(ui_db_pair.surprising);
    }
}
