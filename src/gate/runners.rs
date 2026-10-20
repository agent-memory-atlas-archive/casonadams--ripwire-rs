use crate::domain::{IngestResult, Interner, Lang};
use crate::gate::affected::AffectedTest;
use std::collections::HashMap;
use std::path::Path;

/// The indexed `manifest` file nearest to `test_path`, walking up from its directory to the root.
#[must_use]
pub fn nearest_manifest<'a>(
    test_path: &str,
    manifest: &str,
    indexed: &HashMap<&'a str, Lang>,
) -> Option<&'a str> {
    let mut dir = test_path.rsplit_once('/').map_or("", |(d, _)| d);
    loop {
        let candidate = if dir.is_empty() {
            manifest.to_string()
        } else {
            format!("{dir}/{manifest}")
        };
        if let Some((&found, _)) = indexed.get_key_value(candidate.as_str()) {
            return Some(found);
        }
        if dir.is_empty() {
            return None;
        }
        dir = dir.rsplit_once('/').map_or("", |(d, _)| d);
    }
}

/// Fill each test's `run` from its language's runner rules, reading the test source and nearest manifest.
pub fn attach_test_runners(
    tests: &mut [AffectedTest],
    root: &Path,
    ingest: &IngestResult,
    interner: &Interner,
) {
    let indexed: HashMap<&str, Lang> = ingest
        .files
        .iter()
        .map(|f| (interner.resolve(&f.path), f.lang))
        .collect();
    for test in tests.iter_mut() {
        let Some(&lang) = indexed.get(test.path.as_str()) else {
            continue;
        };
        let semantics = lang.semantics();
        let Ok(source) = crate::domain::read_rel_string(root, &test.path) else {
            continue;
        };
        let manifest = semantics
            .test_runner_manifest()
            .and_then(|name| nearest_manifest(&test.path, name, &indexed))
            .and_then(|p| crate::domain::read_rel_string(root, p).ok());
        test.run = semantics.test_run_command(&test.path, &source, manifest.as_deref());
    }
}
