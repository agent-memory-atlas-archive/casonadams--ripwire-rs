use ripwire_rs::cache::{
    CACHE_VERSION, CachePayload, auto_cache_path, compute_cache_key, load_cached_payload,
};
use ripwire_rs::{Graph, IngestResult, Interner, PageRankResult};

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
