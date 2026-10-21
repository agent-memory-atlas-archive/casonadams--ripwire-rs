use crate::domain::{IngestResult, Interner, Lang, NodeId};
use crate::graph::Graph;
use crate::serialize::XmlEscapeExt;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Write};
use std::path::Path;

/// A test file affected by changes, with its hop distance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedTest {
    pub path: String,
    pub hops: u32,
    pub is_direct_change: bool,
    /// Named after a changed file by test convention; `hops` is 0 unless the graph also reached it.
    pub is_partner: bool,
    /// Shell command that runs this test, when its language's runner is derivable.
    pub run: Option<String>,
    /// A registered shell gate that names a changed file by exact path.
    pub script_literal: bool,
}

impl AffectedTest {
    #[must_use]
    pub fn new(path: String, hops: u32, is_direct_change: bool, is_partner: bool) -> Self {
        Self {
            path,
            hops,
            is_direct_change,
            is_partner,
            run: None,
            script_literal: false,
        }
    }

    /// ` run="CMD"` when a runner is derivable, else ` run_unknown="1"`.
    #[must_use]
    pub fn run_attr(&self) -> String {
        match &self.run {
            Some(cmd) => format!(" run=\"{}\"", cmd.xml_escaped()),
            None => " run_unknown=\"1\"".to_string(),
        }
    }
}

/// Comprehensive affected test reachability report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedReport {
    pub changed_spec: String,
    pub seed_count: usize,
    pub seed_test_files: Vec<String>,
    pub total_tests: usize,
    pub reached_symbols: usize,
    /// Symbols that transitively call the change, seeds excluded, ascending.
    pub impacted: Vec<NodeId>,
    pub tests: Vec<AffectedTest>,
}

impl crate::serialize::XmlReport for AffectedReport {
    crate::xml_report_meta!("affected", "ripwire-rs.affected/v1", root: false);
    fn write_extra_attrs<W: Write>(&self, out: &mut W) -> io::Result<()> {
        write!(
            out,
            " files=\"{}\" seeds=\"{}\" seed_test_files=\"{}\" tests=\"{}\" reached=\"{}\"",
            self.changed_spec,
            self.seed_count,
            self.seed_test_files.len(),
            self.total_tests,
            self.reached_symbols
        )
    }
    fn write_body<W: Write>(&self, out: &mut W) -> io::Result<()> {
        self.tests.iter().try_for_each(|t| t.write_row(out))
    }
}

impl AffectedTest {
    fn write_row<W: Write>(&self, out: &mut W) -> io::Result<()> {
        let partner = if self.is_partner {
            " partner=\"1\""
        } else {
            ""
        };
        let hops = if !self.is_partner || self.hops > 0 {
            format!(" hops=\"{}\"", self.hops)
        } else {
            String::new()
        };
        let direct = if self.is_direct_change {
            " direct=\"1\""
        } else {
            ""
        };
        let run = self.run_attr();
        writeln!(
            out,
            "  <test p=\"{}\"{partner}{hops}{direct}{run}/>",
            self.path.xml_escaped()
        )
    }
}

const PARTNER_SUFFIXES: [&str; 4] = ["_test", "_spec", ".test", ".spec"];

fn can_be_test_file(lang: Lang) -> bool {
    let semantics = lang.semantics();
    semantics.supports_slice() || semantics.is_script_runner()
}

fn universal_partner_subject(stem: &str) -> Option<&str> {
    stem.strip_prefix("test_")
        .or_else(|| PARTNER_SUFFIXES.iter().find_map(|s| stem.strip_suffix(s)))
        .filter(|s| !s.is_empty())
}

fn has_dir_component(p: &Path, pred: impl Fn(&str) -> bool) -> bool {
    p.parent()
        .into_iter()
        .flat_map(Path::components)
        .filter_map(|c| c.as_os_str().to_str())
        .any(|d| pred(&d.to_ascii_lowercase()))
}

/// Check if a path corresponds to a test file by directory convention or filename.
#[must_use]
pub fn is_test_path<P: AsRef<Path>>(path: P) -> bool {
    let p = path.as_ref();
    if has_dir_component(p, |d| {
        matches!(d, "test" | "tests" | "spec" | "specs" | "testdata")
    }) {
        return true;
    }
    let lang = Lang::from_path(p);
    if !can_be_test_file(lang) {
        return false;
    }
    let Some((stem, _)) = p
        .file_name()
        .and_then(|s| s.to_str())
        .and_then(|n| n.rsplit_once('.'))
    else {
        return false;
    };
    universal_partner_subject(stem).is_some()
        || lang.semantics().camel_test_partner_subject(stem).is_some()
}

/// Check if a path is a test, benchmark, or fixture path for ranking and module naming.
#[must_use]
pub fn is_test_or_bench_path<P: AsRef<Path>>(path: P) -> bool {
    let p = path.as_ref();
    if is_test_path(p)
        || has_dir_component(p, |d| {
            matches!(d, "bench" | "benches" | "fixtures" | "__tests__")
                || d.ends_with("_tests")
                || d.ends_with("_test")
        })
    {
        return true;
    }
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    Lang::from_path(p).semantics().is_test_module_stem(stem)
}

/// The source stem a test file at `path` is named after: `test_x`, `x_test`, `x_spec`, `x.test`, `x.spec`
/// in any language, or the language's own convention (e.g. `WidgetTest`) inside a test dir.
#[must_use]
pub fn partner_subject_stem(path: &str, lang: Lang) -> Option<&str> {
    if !can_be_test_file(lang) {
        return None;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    let (stem, _) = name.rsplit_once('.')?;
    universal_partner_subject(stem).or_else(|| {
        has_dir_component(Path::new(path), |d| matches!(d, "test" | "tests"))
            .then(|| lang.semantics().camel_test_partner_subject(stem))
            .flatten()
    })
}

fn file_stem_of(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.split_once('.').map_or(name, |(stem, _)| stem)
}

/// Test files named after any of `changed_paths` by convention, sorted by path; a subject with
/// several candidate tests is ambiguous and gets none.
#[must_use]
pub fn find_partner_tests(
    changed_paths: &[String],
    ingest: &IngestResult,
    interner: &Interner,
) -> Vec<String> {
    let stems: HashSet<&str> = changed_paths.iter().map(|p| file_stem_of(p)).collect();
    let mut by_subject: HashMap<&str, Vec<&str>> = HashMap::new();
    for f in &ingest.files {
        let p = interner.resolve(&f.path);
        if changed_paths.iter().any(|c| c == p) {
            continue;
        }
        if let Some(subject) = partner_subject_stem(p, f.lang).filter(|s| stems.contains(s)) {
            by_subject.entry(subject).or_default().push(p);
        }
    }
    let mut partners: Vec<String> = by_subject
        .into_values()
        .filter(|tests| tests.len() == 1)
        .flatten()
        .map(String::from)
        .collect();
    partners.sort_unstable();
    partners
}

struct SeedResolution {
    seed_nodes: Vec<NodeId>,
    seed_test_files: Vec<String>,
    seed_source_files: Vec<String>,
}

fn push_unique(list: &mut Vec<String>, path: &str) {
    if !list.iter().any(|p| p == path) {
        list.push(path.to_string());
    }
}

fn match_file_seeds(
    f: &crate::domain::FileInfo,
    path_str: &str,
    ingest: &IngestResult,
    seeds: &mut SeedResolution,
) {
    if is_test_path(path_str) {
        push_unique(&mut seeds.seed_test_files, path_str);
        return;
    }
    push_unique(&mut seeds.seed_source_files, path_str);
    seeds.seed_nodes.extend(
        ingest
            .symbols
            .iter()
            .filter(|s| s.file_id == f.id)
            .map(|s| s.id),
    );
}

fn match_symbol_seeds(
    spec: &str,
    ingest: &IngestResult,
    interner: &Interner,
    seeds: &mut SeedResolution,
) {
    for s in &ingest.symbols {
        if interner.resolve(&s.name) == spec {
            seeds.seed_nodes.push(s.id);
        }
    }
}

fn resolve_spec_seeds(
    spec: &str,
    ingest: &IngestResult,
    interner: &Interner,
    seeds: &mut SeedResolution,
) {
    let mut matched_file = false;
    for f in &ingest.files {
        let path_str = interner.resolve(&f.path);
        if path_str == spec || path_str.ends_with(spec) {
            matched_file = true;
            match_file_seeds(f, path_str, ingest, seeds);
        }
    }

    if !matched_file {
        match_symbol_seeds(spec, ingest, interner, seeds);
    }
}

fn traverse_reverse_callers(
    seed_nodes: &[NodeId],
    graph: &Graph,
    ingest: &IngestResult,
    interner: &Interner,
) -> (HashSet<NodeId>, HashMap<String, u32>) {
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut queue: VecDeque<(NodeId, u32)> = VecDeque::new();
    let mut test_hops: HashMap<String, u32> = HashMap::new();

    for &node in seed_nodes {
        visited.insert(node);
        queue.push_back((node, 0));
    }

    while let Some((curr, hops)) = queue.pop_front() {
        if let Some(sym) = ingest.sym(curr) {
            let file_path = ingest.sym_file_path(sym, interner).to_string();
            if is_test_path(&file_path) && hops > 0 {
                test_hops
                    .entry(file_path)
                    .and_modify(|h| *h = (*h).min(hops))
                    .or_insert(hops);
            }
        }

        for (caller, _) in graph.callers(curr) {
            if visited.insert(caller) {
                queue.push_back((caller, hops + 1));
            }
        }
    }

    (visited, test_hops)
}

/// Find test files that transitively reach the changed files or symbols.
#[must_use]
pub fn find_affected_tests(
    changed_specs: &[&str],
    ingest: &IngestResult,
    graph: &Graph,
    interner: &Interner,
) -> AffectedReport {
    let mut seeds = SeedResolution {
        seed_nodes: Vec::new(),
        seed_test_files: Vec::new(),
        seed_source_files: Vec::new(),
    };

    for &spec in changed_specs {
        resolve_spec_seeds(spec.trim(), ingest, interner, &mut seeds);
    }
    seeds.seed_nodes.sort_unstable();
    seeds.seed_nodes.dedup();

    let (visited, mut test_hops) =
        traverse_reverse_callers(&seeds.seed_nodes, graph, ingest, interner);

    let mut tests = Vec::new();
    let mut seen_test_paths = HashSet::new();

    for direct_test in &seeds.seed_test_files {
        seen_test_paths.insert(direct_test.clone());
        tests.push(AffectedTest::new(direct_test.clone(), 0, true, false));
    }

    for partner in find_partner_tests(&seeds.seed_source_files, ingest, interner) {
        if seen_test_paths.insert(partner.clone()) {
            let hops = test_hops.remove(&partner).unwrap_or(0);
            tests.push(AffectedTest::new(partner, hops, false, true));
        }
    }

    for (test_path, hops) in test_hops {
        if seen_test_paths.insert(test_path.clone()) {
            tests.push(AffectedTest::new(test_path, hops, false, false));
        }
    }

    tests.sort_by(|a, b| {
        b.is_direct_change
            .cmp(&a.is_direct_change)
            .then_with(|| b.is_partner.cmp(&a.is_partner))
            .then_with(|| a.hops.cmp(&b.hops))
            .then_with(|| a.path.cmp(&b.path))
    });

    let seed_set: HashSet<NodeId> = seeds.seed_nodes.iter().copied().collect();
    let mut impacted: Vec<NodeId> = visited
        .iter()
        .copied()
        .filter(|n| !seed_set.contains(n))
        .collect();
    impacted.sort_unstable();

    AffectedReport {
        changed_spec: changed_specs.join(","),
        seed_count: seeds.seed_nodes.len(),
        seed_test_files: seeds.seed_test_files,
        total_tests: tests.len(),
        reached_symbols: visited.len(),
        impacted,
        tests,
    }
}
