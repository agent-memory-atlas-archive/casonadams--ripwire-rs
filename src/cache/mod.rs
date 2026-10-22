use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::rank::PageRankResult;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

pub const CACHE_VERSION: u32 = 28;

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

#[derive(Serialize)]
struct CachePayloadRef<'a> {
    version: u32,
    validation_key: String,
    interner_strings: Vec<String>,
    ingest: &'a IngestResult,
    graph: &'a Graph,
    pr: &'a PageRankResult,
}
/// Compute a fast cache validation key from git HEAD (or mtime metadata).
#[must_use]
fn current_binary_stamp() -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    if let Ok(exe) = std::env::current_exe()
        && let Ok(meta) = fs::metadata(exe)
    {
        if let Ok(mtime) = meta.modified() {
            mtime.hash(&mut hasher);
        }
        meta.len().hash(&mut hasher);
    }
    hasher.finish()
}

fn hash_dir_entries(path: &Path, hasher: &mut impl Hasher) {
    for entry in ignore::WalkBuilder::new(path)
        .sort_by_file_path(Ord::cmp)
        .build()
        .flatten()
    {
        entry.path().hash(hasher);
        if let Ok(meta) = entry.metadata() {
            if let Ok(mtime) = meta.modified() {
                mtime.hash(hasher);
            }
            meta.len().hash(hasher);
        }
    }
}

pub fn compute_cache_key(root: &str) -> String {
    let bin_stamp = current_binary_stamp();
    let path = Path::new(root);
    if let Ok(repo) = gix::open(path)
        && let Ok(head) = repo.head_commit()
    {
        return format!("{}-{bin_stamp:x}-{}", env!("CARGO_PKG_VERSION"), head.id());
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bin_stamp.hash(&mut hasher);
    root.hash(&mut hasher);
    hash_dir_entries(path, &mut hasher);
    format!("mtime-{:x}", hasher.finish())
}

/// Derive standard auto-cache path in TMPDIR for a target repository root.
#[must_use]
pub fn auto_cache_path(root: &str) -> PathBuf {
    let abs = fs::canonicalize(root).unwrap_or_else(|_| PathBuf::from(root));
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    abs.to_string_lossy().hash(&mut hasher);
    std::env::temp_dir().join(format!("ripwire-rs-cache-{:x}.json", hasher.finish()))
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

    let bytes = fs::read(path).ok()?;
    load_cached_payload_slice(&bytes, &compute_cache_key(root))
}

/// Load and validate cached payload from a byte slice.
pub fn load_cached_payload_slice(
    bytes: &[u8],
    expected_key: &str,
) -> Option<(IngestResult, Graph, PageRankResult, Interner)> {
    let payload: CachePayload = serde_json::from_slice(bytes).ok()?;
    if payload.version != CACHE_VERSION || payload.validation_key != expected_key {
        return None;
    }
    let interner = Interner::from_strings(&payload.interner_strings);
    Some((payload.ingest, payload.graph, payload.pr, interner))
}

/// Load and validate cached payload from any reader (in-memory testable).
pub fn load_cached_payload<R: std::io::Read>(
    mut reader: R,
    expected_key: &str,
) -> Option<(IngestResult, Graph, PageRankResult, Interner)> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).ok()?;
    load_cached_payload_slice(&bytes, expected_key)
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
    let payload = CachePayloadRef {
        version: CACHE_VERSION,
        validation_key,
        interner_strings: interner.all_strings(),
        ingest,
        graph,
        pr,
    };

    let tmp_path = path.with_extension("tmp");
    let file = File::create(&tmp_path)?;
    let writer = BufWriter::with_capacity(256 * 1024, file);
    serde_json::to_writer(writer, &payload).map_err(|e| io::Error::other(e.to_string()))?;
    fs::rename(tmp_path, path)
}
