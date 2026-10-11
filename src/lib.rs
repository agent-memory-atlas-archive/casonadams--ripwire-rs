pub mod cli;
pub mod crawler;
pub mod domain;
pub mod git;
pub mod graph;
pub mod ingest;
pub mod mcp;
pub mod metrics;
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
pub use git::cochange::{
    check_pair_dependency_capability, mine_cochange_for_file, mine_repo_cochange_pairs, CoPair,
    CoPartner, CochangeOptions, StaticCouplingMap,
};
pub use git::history::{
    civil_from_days, compute_file_churn, days_from_civil, filter_commits_by_scope,
    months_before_epoch, parse_since_scope, walk_repository_history, CommitInfo, FileChurn,
    GitHistoryError, SinceScope, CHURN_DECAY_HALF_LIFE_DAYS, DEFAULT_CHURN_MONTHS,
    DEFAULT_COCHANGE_MONTHS, MERGE_BOMB_MAX_FILES,
};
pub use graph::{CsrMatrix, Graph};
pub use ingest::{ingest_crawled_files, ingest_in_memory_files, LanguageRegistry};
pub use mcp::{dispatch_json_rpc, run_mcp_stdio, run_mcp_stream, McpState};
pub use metrics::{
    compute_complexity, compute_hotspots, compute_readability, posnett_score, ComplexityMetrics,
    HotspotFile, HotspotsReport, ReadabilityMetrics, NESTING_BAR,
};
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
