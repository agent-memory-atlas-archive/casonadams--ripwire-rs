pub mod crawler;
pub mod domain;
pub mod graph;
pub mod ingest;
pub mod resolve;

pub use crawler::{
    crawl_directory, CrawledFile, CrawlerOptions, DEFAULT_MAX_FILE_SIZE, DENYLIST_DIRS,
};
pub use domain::{
    FileInfo, IngestResult, Interner, Lang, NodeId, RefRole, Reference, SymKind, Symbol, NO_NODE,
};
pub use graph::{CsrMatrix, Graph};
pub use ingest::{ingest_crawled_files, LanguageRegistry};
pub use resolve::{
    resolve_all_references, resolve_call_reference, EdgeAcc, ResolutionResult, ResolvedRef,
};
