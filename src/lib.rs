pub mod crawler;
pub mod domain;
pub mod graph;
pub mod ingest;
pub mod rank;
pub mod resolve;
pub mod serialize;

pub use crawler::{
    crawl_directory, CrawledFile, CrawlerOptions, DEFAULT_MAX_FILE_SIZE, DENYLIST_DIRS,
};
pub use domain::{
    FileInfo, IngestResult, Interner, Lang, NodeId, RefRole, Reference, SymKind, Symbol, NO_NODE,
};
pub use graph::{CsrMatrix, Graph};
pub use ingest::{ingest_crawled_files, LanguageRegistry};
pub use rank::{biased_teleport, page_rank, uniform_teleport, PageRankConfig, PageRankResult};
pub use resolve::{
    resolve_all_references, resolve_call_reference, EdgeAcc, ResolutionResult, ResolvedRef,
};
pub use serialize::{
    escape_xml_str, escape_xml_write, serialize_xml, serialize_xml_to_string, SerializeOptions,
};
