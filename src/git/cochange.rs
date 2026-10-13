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
    pub conf_ab: f64,
    pub conf_ba: f64,
    pub driver: &'static str,
    pub recur: u32,
    pub surprising: bool,
    pub dep_capable: bool,
}

/// Fast lookup for static dependency coupling between files.
#[derive(Debug, Default)]
pub struct StaticCouplingMap {
    file_to_id: HashMap<String, u32>,
    direct_adj: Vec<Vec<u32>>,
    memo: std::sync::RwLock<HashMap<u32, HashSet<u32>>>,
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

        Self {
            file_to_id,
            direct_adj,
            memo: std::sync::RwLock::new(HashMap::new()),
            referenced_basenames,
            file_basenames,
        }
    }

    /// Check if two file paths are statically coupled.
    #[must_use]
    pub fn is_coupled(&self, path_a: &str, path_b: &str) -> bool {
        let id_a = match self.file_to_id.get(path_a) {
            Some(&id) => id,
            None => return false,
        };
        let id_b = match self.file_to_id.get(path_b) {
            Some(&id) => id,
            None => return false,
        };
        if id_a == id_b {
            return true;
        }

        if self.names_basename(id_a as usize, id_b as usize)
            || self.names_basename(id_b as usize, id_a as usize)
        {
            return true;
        }

        self.reaches(id_a, id_b) || self.reaches(id_b, id_a)
    }

    fn reaches(&self, src: u32, dst: u32) -> bool {
        if let Ok(guard) = self.memo.read()
            && let Some(closure) = guard.get(&src)
        {
            return closure.contains(&dst);
        }

        let closure = self.compute_closure(src);
        let res = closure.contains(&dst);
        if let Ok(mut guard) = self.memo.write() {
            guard.insert(src, closure);
        }
        res
    }

    fn compute_closure(&self, src: u32) -> HashSet<u32> {
        let mut visited = HashSet::new();
        let mut stack = vec![src];
        visited.insert(src);
        while let Some(curr) = stack.pop() {
            if let Some(neighbors) = self.direct_adj.get(curr as usize) {
                for &next in neighbors {
                    if visited.insert(next) {
                        stack.push(next);
                    }
                }
            }
        }
        visited
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

fn add_callee_file_edges(
    from_node: usize,
    g: &Graph,
    ingest: &IngestResult,
    direct_adj: &mut [Vec<u32>],
) {
    let from_fid = ingest.symbols[from_node].file_id as usize;
    let n_files = direct_adj.len();
    for (to_node, _) in g.callees(from_node as u32) {
        let to_fid = ingest.symbols[to_node as usize].file_id as usize;
        if from_fid != to_fid && from_fid < n_files && to_fid < n_files {
            direct_adj[from_fid].push(to_fid as u32);
        }
    }
}

fn build_direct_file_adj(ingest: &IngestResult, graph: Option<&Graph>) -> Vec<Vec<u32>> {
    let n_files = ingest.files.len();
    let mut direct_adj = vec![Vec::new(); n_files];
    if let Some(g) = graph {
        for from_node in 0..g.node_count() {
            add_callee_file_edges(from_node, g, ingest, &mut direct_adj);
        }
        for adj in &mut direct_adj {
            adj.sort_unstable();
            adj.dedup();
        }
    }
    direct_adj
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
    file_paths: Vec<&'a str>,
    file_counts: Vec<u32>,
    pairs: HashMap<u64, FileCommitStats>,
}

fn tally_repo_pairs<'a>(commits: &[&'a CommitInfo], sub_windows: usize) -> RepoPairsTally<'a> {
    let mut file_map: HashMap<&'a str, u32> = HashMap::new();
    let mut file_paths: Vec<&'a str> = Vec::new();
    let mut file_counts: Vec<u32> = Vec::new();
    let mut pairs: HashMap<u64, FileCommitStats> = HashMap::new();
    let mut commit_ids = Vec::new();

    for (idx, commit) in commits.iter().enumerate() {
        let bit = 1u32 << ((idx * sub_windows) / commits.len());
        commit_ids.clear();
        for f in &commit.changed_files {
            let s = f.as_str();
            let id = *file_map.entry(s).or_insert_with(|| {
                let new_id = file_paths.len() as u32;
                file_paths.push(s);
                file_counts.push(0);
                new_id
            });
            file_counts[id as usize] += 1;
            commit_ids.push(id);
        }

        commit_ids.sort_unstable();
        commit_ids.dedup();

        let n = commit_ids.len();
        for i in 0..n {
            let lo = commit_ids[i] as u64;
            for &hi in &commit_ids[i + 1..] {
                let key = (lo << 32) | (hi as u64);
                let entry = pairs.entry(key).or_insert(FileCommitStats {
                    together: 0,
                    recur_mask: 0,
                });
                entry.together += 1;
                entry.recur_mask |= bit;
            }
        }
    }
    RepoPairsTally {
        file_paths,
        file_counts,
        pairs,
    }
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
    for (&key, stats) in &tally.pairs {
        if stats.together >= opts.min_support
            && let Some(p) = build_copair(key, stats, &tally, coupling, ingest, interner)
        {
            pairs.push(p);
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

fn order_pair_conf(
    fa: &str,
    fb: &str,
    deg_a: f64,
    deg_b: f64,
) -> (String, String, f64, f64, &'static str) {
    if fa < fb {
        let driver = if deg_b >= deg_a { "b" } else { "a" };
        (fa.to_string(), fb.to_string(), deg_a, deg_b, driver)
    } else {
        let driver = if deg_a >= deg_b { "b" } else { "a" };
        (fb.to_string(), fa.to_string(), deg_b, deg_a, driver)
    }
}

fn build_copair(
    key: u64,
    stats: &FileCommitStats,
    tally: &RepoPairsTally<'_>,
    coupling: Option<&StaticCouplingMap>,
    ingest: Option<&IngestResult>,
    interner: Option<&Interner>,
) -> Option<CoPair> {
    let id_a = (key >> 32) as usize;
    let id_b = (key as u32) as usize;
    let fa = tally.file_paths[id_a];
    let fb = tally.file_paths[id_b];
    if !is_file_path(fa) || !is_file_path(fb) {
        return None;
    }
    let total_a = tally.file_counts[id_a];
    let total_b = tally.file_counts[id_b];
    let deg_a = (stats.together as f64) / (total_a as f64);
    let deg_b = (stats.together as f64) / (total_b as f64);
    let deg = deg_a.max(deg_b);
    let recur = stats.recur_mask.count_ones();
    let dep_capable = check_pair_dependency_capability(fa, fb, ingest, interner);
    let is_coupled = coupling.is_some_and(|c| c.is_coupled(fa, fb));
    let surprising = dep_capable && !is_coupled;
    let (file_a, file_b, conf_ab, conf_ba, driver) = order_pair_conf(fa, fb, deg_a, deg_b);
    Some(CoPair {
        file_a,
        file_b,
        together: stats.together,
        deg,
        conf_ab,
        conf_ba,
        driver,
        recur,
        surprising,
        dep_capable,
    })
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
