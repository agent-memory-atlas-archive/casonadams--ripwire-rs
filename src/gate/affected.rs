use crate::domain::{IngestResult, Interner, NodeId};
use crate::graph::Graph;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Write};
use std::path::Path;

/// A test file affected by changes, with its hop distance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedTest {
    pub path: String,
    pub hops: u32,
    pub is_direct_change: bool,
}

/// Comprehensive affected test reachability report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedReport {
    pub changed_spec: String,
    pub seed_count: usize,
    pub seed_test_files: Vec<String>,
    pub total_tests: usize,
    pub reached_symbols: usize,
    pub tests: Vec<AffectedTest>,
}

impl crate::serialize::XmlReport for AffectedReport {
    fn schema_tag(&self) -> &'static str {
        "affected"
    }
    fn schema_version(&self) -> &'static str {
        "ripwire.affected/v1"
    }
    fn include_root_attr(&self) -> bool {
        false
    }
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
        for t in &self.tests {
            let direct_attr = if t.is_direct_change {
                " direct=\"1\""
            } else {
                ""
            };
            writeln!(
                out,
                "  <test p=\"{}\" hops=\"{}\"{}/>",
                t.path, t.hops, direct_attr
            )?;
        }
        Ok(())
    }
}

/// Check if a path corresponds to a test file by directory convention or filename.
#[must_use]
pub fn is_test_path(path: &str) -> bool {
    let p = Path::new(path);
    for component in p.components() {
        if let Some(s) = component.as_os_str().to_str() {
            let lower = s.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "test" | "tests" | "spec" | "specs" | "testdata"
            ) {
                return true;
            }
        }
    }

    let file_name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let lower = file_name.to_ascii_lowercase();
    lower.starts_with("test_")
        || lower.ends_with("_test.rs")
        || lower.ends_with("_test.go")
        || lower.ends_with(".test.ts")
        || lower.ends_with(".test.js")
        || lower.ends_with(".spec.ts")
        || lower.ends_with(".spec.js")
        || lower.ends_with("test.java")
}

struct SeedResolution {
    seed_nodes: Vec<NodeId>,
    seed_test_files: Vec<String>,
}

fn match_file_seeds(
    f: &crate::domain::FileInfo,
    path_str: &str,
    ingest: &IngestResult,
    seeds: &mut SeedResolution,
) {
    if is_test_path(path_str) {
        if !seeds.seed_test_files.contains(&path_str.to_string()) {
            seeds.seed_test_files.push(path_str.to_string());
        }
    } else {
        for s in &ingest.symbols {
            if s.file_id == f.id {
                seeds.seed_nodes.push(s.id);
            }
        }
    }
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

fn collect_module_scope_test_callers(
    node_hops: &HashMap<NodeId, u32>,
    ingest: &IngestResult,
    interner: &Interner,
    test_hops: &mut HashMap<String, u32>,
) {
    let mut visited_by_name: HashMap<lasso::Spur, (u32, crate::domain::Lang)> = HashMap::new();
    for (&nid, &hops) in node_hops {
        if let Some(sym) = ingest.sym(nid) {
            visited_by_name
                .entry(sym.name)
                .and_modify(|e| {
                    if hops < e.0 {
                        *e = (hops, sym.lang);
                    }
                })
                .or_insert((hops, sym.lang));
        }
    }
    for r in &ingest.references {
        if r.from_symbol != crate::domain::NO_NODE {
            continue;
        }
        let Some(&(d, sym_lang)) = visited_by_name.get(&r.callee_name) else {
            continue;
        };
        if !sym_lang.is_compatible(r.lang) {
            continue;
        }
        if let Some(f) = ingest.file(r.file_id) {
            let file_path = &interner[f.path];
            if is_test_path(file_path) {
                let h = d + 1;
                test_hops
                    .entry(file_path.to_string())
                    .and_modify(|cur| *cur = (*cur).min(h))
                    .or_insert(h);
            }
        }
    }
}

fn traverse_reverse_callers(
    seed_nodes: &[NodeId],
    graph: &Graph,
    ingest: &IngestResult,
    interner: &Interner,
) -> (HashSet<NodeId>, HashMap<String, u32>) {
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut node_hops: HashMap<NodeId, u32> = HashMap::new();
    let mut queue: VecDeque<(NodeId, u32)> = VecDeque::new();
    let mut test_hops: HashMap<String, u32> = HashMap::new();

    for &node in seed_nodes {
        visited.insert(node);
        node_hops.insert(node, 0);
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
                node_hops.insert(caller, hops + 1);
                queue.push_back((caller, hops + 1));
            }
        }
    }

    collect_module_scope_test_callers(&node_hops, ingest, interner, &mut test_hops);
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
    };

    for &spec in changed_specs {
        resolve_spec_seeds(spec.trim(), ingest, interner, &mut seeds);
    }
    seeds.seed_nodes.sort_unstable();
    seeds.seed_nodes.dedup();

    let (visited, test_hops) = traverse_reverse_callers(&seeds.seed_nodes, graph, ingest, interner);

    let mut tests = Vec::new();
    let mut seen_test_paths = HashSet::new();

    for direct_test in &seeds.seed_test_files {
        seen_test_paths.insert(direct_test.clone());
        tests.push(AffectedTest {
            path: direct_test.clone(),
            hops: 0,
            is_direct_change: true,
        });
    }

    for (test_path, hops) in test_hops {
        if seen_test_paths.insert(test_path.clone()) {
            tests.push(AffectedTest {
                path: test_path,
                hops,
                is_direct_change: false,
            });
        }
    }

    tests.sort_by(|a, b| {
        b.is_direct_change
            .cmp(&a.is_direct_change)
            .then_with(|| a.hops.cmp(&b.hops))
            .then_with(|| a.path.cmp(&b.path))
    });

    AffectedReport {
        changed_spec: changed_specs.join(","),
        seed_count: seeds.seed_nodes.len(),
        seed_test_files: seeds.seed_test_files,
        total_tests: tests.len(),
        reached_symbols: visited.len(),
        tests,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FileInfo, Lang, RefRole, SymKind, Symbol};
    use crate::serialize::ToXmlString;

    #[test]
    fn test_is_test_path_detection() {
        assert!(is_test_path("tests/foo.rs"));
        assert!(is_test_path("src/lib_test.go"));
        assert!(is_test_path("ui/component.test.ts"));
        assert!(is_test_path("core/test_engine.rs"));
        assert!(!is_test_path("src/lib.rs"));
        assert!(!is_test_path("src/engine.rs"));
    }

    #[test]
    fn test_affected_transitive_reachability() {
        let interner = Interner::new();
        let mut ingest = IngestResult::new();

        let p_core = interner.intern("src/core.rs");
        let p_service = interner.intern("src/service.rs");
        let p_test = interner.intern("tests/test_service.rs");

        ingest.files.push(FileInfo {
            id: 0,
            path: p_core,
            lang: Lang::Rust,
            line_count: 50,
            byte_size: 1000,
        });
        ingest.files.push(FileInfo {
            id: 1,
            path: p_service,
            lang: Lang::Rust,
            line_count: 50,
            byte_size: 1000,
        });
        ingest.files.push(FileInfo {
            id: 2,
            path: p_test,
            lang: Lang::Rust,
            line_count: 50,
            byte_size: 1000,
        });

        // Symbol 0: core_fn in src/core.rs
        let s0 = Symbol::new(
            0,
            interner.intern("core_fn"),
            interner.intern("core_fn"),
            SymKind::Function,
            Lang::Rust,
            0,
            5,
        );

        // Symbol 1: service_fn in src/service.rs -> calls core_fn
        let s1 = Symbol::new(
            1,
            interner.intern("service_fn"),
            interner.intern("service_fn"),
            SymKind::Function,
            Lang::Rust,
            1,
            10,
        );

        // Symbol 2: test_fn in tests/test_service.rs -> calls service_fn
        let s2 = Symbol::new(
            2,
            interner.intern("test_fn"),
            interner.intern("test_fn"),
            SymKind::Function,
            Lang::Rust,
            2,
            15,
        );

        ingest.symbols.push(s0);
        ingest.symbols.push(s1);
        ingest.symbols.push(s2);

        // Reference 1: service_fn -> core_fn
        ingest.references.push(crate::domain::Reference::new(
            1,
            interner.intern("core_fn"),
            1,
            12,
            Lang::Rust,
            RefRole::Call,
        ));

        // Reference 2: test_fn -> service_fn
        ingest.references.push(crate::domain::Reference::new(
            2,
            interner.intern("service_fn"),
            2,
            18,
            Lang::Rust,
            RefRole::Call,
        ));

        let graph = Graph::build(&ingest, &interner);

        // Query affected tests when modifying "src/core.rs"
        let report = find_affected_tests(&["src/core.rs"], &ingest, &graph, &interner);
        assert_eq!(report.seed_count, 1);
        assert_eq!(report.total_tests, 1);
        assert_eq!(report.tests[0].path, "tests/test_service.rs");
        assert_eq!(report.tests[0].hops, 2); // core_fn <- service_fn <- test_fn

        let xml = report.to_xml_string();
        assert!(xml.contains(
            "<affected schema=\"ripwire.affected/v1\" files=\"src/core.rs\" seeds=\"1\""
        ));
        assert!(xml.contains("<test p=\"tests/test_service.rs\" hops=\"2\"/>"));
    }
}
