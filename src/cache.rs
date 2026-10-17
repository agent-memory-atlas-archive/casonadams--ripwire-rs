use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::rank::PageRankResult;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{self, BufReader, BufWriter};
use std::path::{Path, PathBuf};

const CACHE_VERSION: u32 = 8;

/// Serialized binary cache payload for fast warm repository queries.
#[derive(Serialize, Deserialize)]
pub struct CachePayload {
    pub version: u32,
    pub validation_key: String,
    pub interner_strings: Vec<String>,
    pub ingest: IngestResult,
    pub graph: Graph,
    pub pr: PageRankResult,
}

/// Compute a fast cache validation key from git HEAD (or mtime metadata).
#[must_use]
pub fn compute_cache_key(root: &str) -> String {
    let path = Path::new(root);
    if let Ok(repo) = gix::open(path)
        && let Ok(head) = repo.head_commit()
    {
        return format!("{}-{}", env!("CARGO_PKG_VERSION"), head.id());
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hasher);
    if let Ok(meta) = fs::metadata(path) {
        if let Ok(mtime) = meta.modified() {
            mtime.hash(&mut hasher);
        }
        meta.len().hash(&mut hasher);
    }
    format!("mtime-{:x}", hasher.finish())
}

/// Derive standard auto-cache path in TMPDIR for a target repository root.
#[must_use]
pub fn auto_cache_path(root: &str) -> PathBuf {
    let abs = fs::canonicalize(root).unwrap_or_else(|_| PathBuf::from(root));
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    abs.to_string_lossy().hash(&mut hasher);
    std::env::temp_dir().join(format!("ripwire-cache-{:x}.json", hasher.finish()))
}

/// Attempt to load cached repository analysis if validation key matches.
pub fn load_cached_analysis(
    root: &str,
    cache_path: Option<&Path>,
) -> Option<(IngestResult, Graph, PageRankResult, Interner)> {
    let path_buf;
    let path = match cache_path {
        Some(p) => p,
        None => {
            path_buf = auto_cache_path(root);
            &path_buf
        }
    };

    let file = File::open(path).ok()?;
    load_cached_payload(BufReader::new(file), &compute_cache_key(root))
}

/// Load and validate cached payload from any reader (in-memory testable).
pub fn load_cached_payload<R: std::io::Read>(
    reader: R,
    expected_key: &str,
) -> Option<(IngestResult, Graph, PageRankResult, Interner)> {
    let payload: CachePayload = serde_json::from_reader(reader).ok()?;
    if payload.version != CACHE_VERSION || payload.validation_key != expected_key {
        return None;
    }
    let interner = Interner::from_strings(&payload.interner_strings);
    Some((payload.ingest, payload.graph, payload.pr, interner))
}

/// Save repository analysis to cache file atomically.
pub fn save_cached_analysis(
    root: &str,
    cache_path: Option<&Path>,
    analysis: (&IngestResult, &Graph, &PageRankResult, &Interner),
) -> io::Result<()> {
    let (ingest, graph, pr, interner) = analysis;
    let path_buf;
    let path = match cache_path {
        Some(p) => p,
        None => {
            path_buf = auto_cache_path(root);
            &path_buf
        }
    };

    let validation_key = compute_cache_key(root);
    let payload = CachePayload {
        version: CACHE_VERSION,
        validation_key,
        interner_strings: interner.all_strings(),
        ingest: ingest.clone(),
        graph: graph.clone(),
        pr: pr.clone(),
    };

    let tmp_path = path.with_extension("tmp");
    let file = File::create(&tmp_path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer(writer, &payload).map_err(|e| io::Error::other(e.to_string()))?;
    fs::rename(tmp_path, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_payload_roundtrip() {
        let interner = Interner::new();
        interner.intern("test_symbol");
        interner.intern("src/test.rs");

        let payload = CachePayload {
            version: CACHE_VERSION,
            validation_key: "test_key_123".into(),
            interner_strings: interner.all_strings(),
            ingest: IngestResult::default(),
            graph: Graph::default(),
            pr: PageRankResult {
                iterations: 5,
                converged: true,
                ranks: vec![1.0],
            },
        };

        let json = serde_json::to_string(&payload).unwrap();
        let loaded: CachePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.version, CACHE_VERSION);
        assert_eq!(loaded.validation_key, "test_key_123");
        assert_eq!(loaded.interner_strings.len(), 2);
    }
    #[test]
    fn test_load_cached_payload_in_memory() {
        let interner = Interner::new();
        let payload = CachePayload {
            version: CACHE_VERSION,
            validation_key: "valid_key".into(),
            interner_strings: interner.all_strings(),
            ingest: IngestResult::default(),
            graph: Graph::default(),
            pr: PageRankResult {
                iterations: 1,
                converged: true,
                ranks: vec![],
            },
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(load_cached_payload(std::io::Cursor::new(json.as_bytes()), "valid_key").is_some());
        assert!(load_cached_payload(std::io::Cursor::new(json.as_bytes()), "wrong_key").is_none());
    }

    #[test]
    fn test_compute_cache_key_non_empty() {
        let key = compute_cache_key(".");
        assert!(!key.is_empty());
        let path = auto_cache_path(".");
        assert!(path.to_string_lossy().contains("ripwire-cache-"));
    }
}
