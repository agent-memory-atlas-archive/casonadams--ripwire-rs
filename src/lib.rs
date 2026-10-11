pub mod cli;
pub mod clones;
pub mod crawler;
pub mod domain;
pub mod gate;
pub mod git;
pub mod graph;
pub mod ingest;
pub mod mcp;
pub mod metrics;
pub mod navigate;
pub mod rank;
pub mod resolve;
pub mod serialize;
pub mod slice;

pub use cli::{run_cli, run_cli_args, Cli, Commands};
pub use clones::{
    execute_clones_command, find_clones, render_clones_xml, CloneGroup, CloneIdiom, CloneType,
    DEFAULT_MIN_CLONE_TOKENS,
};
pub use crawler::{
    crawl_directory, CrawledFile, CrawlerOptions, DEFAULT_MAX_FILE_SIZE, DENYLIST_DIRS,
};
pub use domain::{
    FileInfo, IngestResult, Interner, Lang, NodeId, RefRole, Reference, SymKind, Symbol, NO_NODE,
};
pub use gate::{
    check_symbol_contract, compare_contracts, compute_quality_delta, compute_snapshot,
    evaluate_quality_delta, find_affected_tests, is_test_path, load_head_symbol_contract,
    read_baseline_file, write_baseline_file, AffectedReport, AffectedTest, CallerCheckResult,
    ContractStatus, EditCheckReport, IncompatibleCallSite, QualityDeltaReport, QualityRegression,
    QualitySnapshot, SignatureContract, DEFAULT_BASELINE_FILE,
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
    escape_xml_str, escape_xml_write, generate_call_graph_html, serialize_xml,
    serialize_xml_to_string, write_call_graph_html, ColorBy, SerializeOptions, ToXmlString,
};
pub use slice::{
    compute_dataflow_slice, execute_slice, parse_slice_target, OccRole, SliceFlowDir, SliceStep,
    VariableOcc,
};
