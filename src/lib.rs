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

pub use cli::{Cli, Commands, run_cli, run_cli_args};
pub use clones::{
    CloneGroup, CloneIdiom, CloneType, DEFAULT_MIN_CLONE_TOKENS, execute_clones_command,
    find_clones, render_clones_xml,
};
pub use crawler::{
    CrawledFile, CrawlerOptions, DEFAULT_MAX_FILE_SIZE, DENYLIST_DIRS, crawl_directory,
};
pub use domain::{
    FileInfo, IngestResult, Interner, Lang, NO_NODE, NodeId, RefRole, Reference, SymKind, Symbol,
};
pub use gate::{
    AffectedReport, AffectedTest, CallerCheckResult, ContractStatus, DEFAULT_BASELINE_FILE,
    EditCheckReport, IncompatibleCallSite, QualityDeltaReport, QualityRegression, QualitySnapshot,
    SignatureContract, check_symbol_contract, compare_contracts, compute_quality_delta,
    compute_snapshot, evaluate_quality_delta, find_affected_tests, is_test_path,
    load_head_symbol_contract, read_baseline_file, write_baseline_file,
};
pub use git::cochange::{
    CoPair, CoPartner, CochangeOptions, StaticCouplingMap, check_pair_dependency_capability,
    mine_cochange_for_file, mine_repo_cochange_pairs,
};
pub use git::history::{
    CHURN_DECAY_HALF_LIFE_DAYS, CommitInfo, DEFAULT_CHURN_MONTHS, DEFAULT_COCHANGE_MONTHS,
    FileChurn, GitHistoryError, MERGE_BOMB_MAX_FILES, SinceScope, civil_from_days,
    compute_file_churn, days_from_civil, filter_commits_by_scope, months_before_epoch,
    parse_since_scope, walk_repository_history,
};
pub use graph::{CsrMatrix, Graph};
pub use ingest::{LanguageRegistry, ingest_crawled_files, ingest_in_memory_files};
pub use mcp::{McpState, dispatch_json_rpc, run_mcp_stdio, run_mcp_stream};
pub use metrics::{
    ComplexityMetrics, HotspotFile, HotspotsReport, NESTING_BAR, ReadabilityMetrics,
    compute_complexity, compute_hotspots, compute_readability, posnett_score,
};
pub use navigate::{
    ImpactNode, NavNeighborhood, SymbolNavInfo, find_symbols_by_name, get_symbol_nav_info,
    navigate_1hop, serialize_callees_xml, serialize_callers_xml, serialize_find_symbol_xml,
    serialize_impact_xml, serialize_uses_xml, transitive_impact,
};
pub use rank::{
    HitsConfig, HitsResult, PageRankConfig, PageRankResult, biased_teleport, compute_hits,
    page_rank, uniform_teleport,
};
pub use resolve::{
    EdgeAcc, ResolutionResult, ResolvedRef, resolve_all_references, resolve_call_reference,
};
pub use serialize::{
    ColorBy, CountingWriter, DEFAULT_PACK_BUDGET_BYTES, RootElementContext, SerializeOptions,
    StreamXmlContext, ToXmlString, escape_xml_str, escape_xml_write, generate_call_graph_html,
    serialize_xml, serialize_xml_to_string, serialize_xml_with_ranks, write_call_graph_html,
};
pub use slice::{
    OccRole, SliceFlowDir, SliceStep, VariableOcc, compute_dataflow_slice, execute_slice,
    parse_slice_target,
};
