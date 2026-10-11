pub mod cli;
pub mod crawler;
pub mod domain;
pub mod graph;
pub mod ingest;
pub mod mcp;
pub mod navigate;
pub mod rank;
pub mod resolve;
pub mod serialize;

pub use cli::{run_cli, run_cli_args, Cli, Commands};
pub use crawler::{
    crawl_directory, CrawledFile, CrawlerOptions, DEFAULT_MAX_FILE_SIZE, DENYLIST_DIRS,
};
pub use domain::{
    FileInfo, IngestResult, Interner, Lang, NodeId, RefRole, Reference, SymKind, Symbol, NO_NODE,
};
pub use graph::{CsrMatrix, Graph};
pub use ingest::{ingest_crawled_files, ingest_in_memory_files, LanguageRegistry};
pub use mcp::{dispatch_json_rpc, run_mcp_stdio, run_mcp_stream, McpState};
pub use navigate::{
    find_symbols_by_name, navigate_1hop, transitive_impact, ImpactNode, NavNeighborhood,
    SymbolNavInfo,
};
pub use rank::{biased_teleport, page_rank, uniform_teleport, PageRankConfig, PageRankResult};
pub use resolve::{
    resolve_all_references, resolve_call_reference, EdgeAcc, ResolutionResult, ResolvedRef,
};
pub use serialize::{
    escape_xml_str, escape_xml_write, serialize_xml, serialize_xml_to_string, SerializeOptions,
};
