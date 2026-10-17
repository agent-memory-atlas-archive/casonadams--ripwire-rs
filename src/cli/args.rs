use clap::{Args, Parser, Subcommand};

/// CLI command line options.
#[derive(Parser, Debug, Default, PartialEq)]
#[command(
    name = "ripwire-rs",
    version,
    about = "High-throughput agent context router and static analysis engine"
)]
pub struct Cli {
    /// Target repository path (default: current directory)
    #[arg(value_name = "PATH")]
    pub path: Option<String>,
    /// Fowler's Shotgun Surgery co-change mining
    #[arg(long)]
    pub cochange: Option<Option<String>>,
    /// Limit git history traversal to commits after this date or revision
    #[arg(long)]
    pub since: Option<String>,
    /// Show top maintenance hotspots (churn * ccx)
    #[arg(long)]
    pub hotspots: bool,
    /// Verify 1-hop caller compatibility against git HEAD
    #[arg(long)]
    pub edit_check: Option<String>,
    /// Show test files affected by changes
    #[arg(long)]
    pub affected: Option<String>,
    /// Pin quality baseline stamped with git HEAD
    #[arg(long)]
    pub quality_baseline: bool,
    /// Report code quality delta regressions vs baseline
    #[arg(long)]
    pub quality_delta: bool,
    /// Detect AST code duplication and clones (Type-1, Type-2, Type-3)
    #[arg(long)]
    pub clones: bool,
    /// Minimum token length for clone detection
    #[arg(long, default_value_t = 40)]
    pub min_tokens: usize,
    /// ARISE intra-procedural def-use variable slice (SYM[:VAR])
    #[arg(long)]
    pub slice: Option<String>,
    /// Dataflow direction for --slice (fwd, back, both)
    #[arg(long, default_value = "fwd")]
    pub slice_flow: String,
    /// Max hop depth for --slice
    #[arg(long = "slice-depth", default_value_t = 8)]
    pub slice_depth: u32,
    #[arg(long, value_name = "FILE")]
    pub html: Option<Option<String>>,
    /// Node coloring mode for --html: lang, community, cx, churn, tested
    #[arg(long, default_value = "lang")]
    pub color_by: String,
    /// Ranking signal: pagerank, authority, hub
    #[arg(long, default_value = "pagerank")]
    pub rank_by: String,
    /// Streaming budget in bytes (default 65536)
    #[arg(
        long = "pack-budget",
        visible_alias = "pack-budget-bytes",
        value_name = "BYTES"
    )]
    pub pack_budget: Option<usize>,
    /// Token budget (~4 bytes per token, alternative to --pack-budget)
    #[arg(long)]
    pub token_budget: Option<usize>,
    /// Budget the map to ~N tokens (~4 bytes per token, alias for --token-budget)
    #[arg(long = "max-tokens", value_name = "N")]
    pub max_tokens: Option<usize>,
    /// Keep only the N highest-ranked symbols (e.g. 200)
    #[arg(long = "top-k", value_name = "N")]
    pub top_k: Option<usize>,
    /// Schema legend posture: compact (default, in-context schema comment) or none
    #[arg(long, default_value = "compact")]
    pub legend: String,
    /// Compact brief report mode
    #[arg(long = "brief")]
    pub brief: bool,
    /// Overlay SCIP compiler-backed index
    #[arg(long = "scip", value_name = "FILE")]
    pub scip: Option<String>,

    /// Top-level symbol navigation flags
    #[command(flatten)]
    pub nav: TopNavArgs,
    /// Top-level retrieval and architecture flags
    #[command(flatten)]
    pub retrieval: TopRetrievalArgs,
    /// Top-level workspace and audit flags
    #[command(flatten)]
    pub workspace: TopWorkspaceArgs,
    /// Top-level detail ladder flags
    #[command(flatten)]
    pub detail: TopDetailArgs,
    /// Top-level pre-PR safety flags
    #[command(flatten)]
    pub safety: TopSafetyArgs,
    /// Top-level architecture and discovery flags
    #[command(flatten)]
    pub arch: TopArchitectureArgs,
    /// Top-level global modifiers and batching flags
    #[command(flatten)]
    pub modifiers: TopModifierArgs,
    /// Launch stdio JSON-RPC Model Context Protocol server
    #[arg(long = "mcp")]
    pub mcp: bool,
    /// Install ripwire-rs agent skills to ~/.agents/skills (or custom path)
    #[arg(
        long = "add-skills",
        visible_alias = "install-skills",
        value_name = "DIR"
    )]
    pub add_skills: Option<Option<String>>,
    /// Workflow subcommand group
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Top-level symbol navigation flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopNavArgs {
    /// Query 1-hop callers of a symbol
    #[arg(long, value_name = "SYM")]
    pub callers: Option<String>,
    /// Query 1-hop callees of a symbol
    #[arg(long, value_name = "SYM")]
    pub callees: Option<String>,
    /// Query transitive impact blast radius of a symbol
    #[arg(long, value_name = "SYM")]
    pub impact: Option<String>,
    /// Query all static use-sites of a symbol
    #[arg(long, value_name = "SYM")]
    pub uses: Option<String>,
    /// Query 1-hop callers and callees of a symbol
    #[arg(long = "find-symbol", value_name = "SYM")]
    pub find_symbol: Option<String>,
}

/// Top-level detail ladder flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopDetailArgs {
    /// Expand full symbol bodies and callee signatures (SYM,SYM...)
    #[arg(long, value_name = "SYMS")]
    pub expand: Option<String>,
    #[arg(long, value_name = "SYMS")]
    pub outline: Option<String>,
    /// Pack bodies of the top N highest-ranked symbols
    #[arg(long = "pack-top-n", value_name = "N")]
    pub pack_top_n: Option<usize>,
    /// Orient file by file showing each file's top symbols
    #[arg(long)]
    pub tree: bool,
    /// Rank functions by readability volume and size, largest first
    #[arg(long = "biggest-first")]
    pub biggest_first: bool,
    /// Reachable nonlocal mutable state read or written per function
    #[arg(long = "nonlocal-state")]
    pub nonlocal_state: bool,
    /// Scan codebase for AST code smells across all symbols
    #[arg(long = "lint")]
    pub lint: bool,
    /// Print built-in AST code smell catalog
    #[arg(long = "lint-catalog")]
    pub lint_catalog: bool,
    /// External YAML ast-grep lint rules directory
    #[arg(long = "lint-rules", value_name = "DIR")]
    pub lint_rules: Option<String>,
    /// Select specific lint rules to run
    #[arg(long = "lint-select", value_name = "RULES")]
    pub lint_select: Option<String>,
    /// Ignore specific lint rules
    #[arg(long = "lint-ignore", value_name = "RULES")]
    pub lint_ignore: Option<String>,
    /// Maximum findings emitted per lint rule
    #[arg(long = "lint-max-per-rule", value_name = "N")]
    pub lint_max_per_rule: Option<usize>,
    /// Check local-variable naming consistency
    #[arg(long = "naming-locals")]
    pub naming_locals: bool,
    /// Naming convention consistency check
    #[arg(long = "naming-consistency")]
    pub naming_consistency: bool,
    /// Calibrate naming lens rules against git rename history
    #[arg(long = "naming-calibration")]
    pub naming_calibration: bool,
    /// Generate executive markdown architecture report
    #[arg(long = "report")]
    pub report: bool,
    /// Emit full schema legend dictionary
    #[arg(long = "legend-dict")]
    pub legend_dict: bool,
}
impl TopDetailArgs {
    pub fn is_active(&self) -> bool {
        self.expand.is_some()
            || self.outline.is_some()
            || self.pack_top_n.is_some()
            || self.tree
            || self.biggest_first
            || self.nonlocal_state
            || self.lint
            || self.lint_catalog
            || self.naming_locals
            || self.naming_consistency
            || self.naming_calibration
            || self.report
            || self.legend_dict
    }
}

/// Top-level pre-PR safety and verification flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopSafetyArgs {
    /// Before a PR: name tests to run and untested blast radius (exit 4 if non-empty)
    #[arg(long = "test-gate", value_name = "FILES")]
    pub test_gate: Option<Option<String>>,
    /// Situational awareness for a change: blast radius + tests + co-change (default: git diff)
    #[arg(long, value_name = "FILES")]
    pub situ: Option<Option<String>>,
    /// Safe delete assessment joining callers, blast radius, tests, and history
    #[arg(long = "safe-delete", value_name = "SYM")]
    pub safe_delete: Option<String>,
    /// Show what a test file actually covers: non-test symbols it reaches
    #[arg(long, value_name = "TESTFILE")]
    pub exercises: Option<String>,
    /// Re-rank map toward git-changed files so recent work floats up
    #[arg(long = "map-diff")]
    pub map_diff: bool,
    /// Cross-directory call edges lacking test coverage
    #[arg(long = "seams")]
    pub seams: bool,
    /// Review bundle per changed file vs base: symbols, callers, blast radius, tests
    #[arg(long = "pr-context", value_name = "REF")]
    pub pr_context: Option<Option<String>>,
    /// Cross-branch symbol conflict predictions across concurrent branch refs
    #[arg(long = "merge-scout", value_name = "REF1,REF2,...")]
    pub merge_scout: Option<String>,
    /// Acknowledge findings in quality-delta
    #[arg(long = "quality-ack", value_name = "REASON")]
    pub quality_ack: Option<Option<String>>,
    /// Narrow quality-ack to specific kinds or facets
    #[arg(long = "ack-only", value_name = "SUBSTR")]
    pub ack_only: Option<String>,
    /// Allow quality-baseline on a dirty working tree
    #[arg(long = "allow-dirty")]
    pub allow_dirty: bool,
    /// Show stale quality-delta acknowledgments even on clean runs
    #[arg(long = "show-stale")]
    pub show_stale: bool,
    /// Partition quality-delta by ownership scope
    #[arg(long = "scope", value_name = "SCOPE")]
    pub scope: Option<String>,
    /// Pin architectural baseline sidecar
    #[arg(long = "baseline")]
    pub baseline: bool,
    /// Update architectural baseline sidecar with new violations
    #[arg(long = "baseline-update")]
    pub baseline_update: bool,
    /// Cluster co-changing files into multi-file change groups
    #[arg(long = "cochange-groups")]
    pub cochange_groups: bool,
    /// Minimum sub-window recurrence count for co-change pairs
    #[arg(long = "cochange-recur", value_name = "K")]
    pub cochange_recur: Option<u32>,
    /// Target file for symbol edit disambiguation
    #[arg(long = "edit-target-file", value_name = "FILE")]
    pub edit_target_file: Option<String>,
    /// Skip post-edit contract and syntax verification
    #[arg(long = "no-post-check")]
    pub no_post_check: bool,
    /// Allow atomic symbol edits when connected over remote transport
    #[arg(long = "allow-remote-edits")]
    pub allow_remote_edits: bool,
    /// Pin quality census snapshot
    #[arg(long = "pin-census")]
    pub pin_census: bool,
    /// Partition codebase into cohesive modules
    #[arg(long = "partition", value_name = "K")]
    pub partition: Option<Option<usize>>,
    /// Delta Maintainability Model scalar in [0,1]
    #[arg(long = "dmm", num_args = 0..=1, default_missing_value = "", value_name = "REV|A..B")]
    pub dmm: Option<Option<String>>,
    /// Plan parallel worktree lanes before code (split across N worktrees)
    #[arg(long = "plan-lanes", num_args = 0..=1, default_missing_value = "2", value_name = "N")]
    pub plan_lanes: Option<Option<usize>>,
    /// Goal or task description for --plan-lanes
    #[arg(long = "task", value_name = "GOAL")]
    pub task: Option<String>,
    /// Security audit scanner for agent skill files
    #[arg(
        long = "scan-skills",
        visible_alias = "scan-skill",
        num_args = 0..=1,
        default_missing_value = "",
        value_name = "DIR"
    )]
    pub scan_skills: Option<Option<String>>,
    /// Apply atomic code mutations
    #[arg(long = "apply")]
    pub apply: bool,
    /// Validate edits without mutating files
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    /// Force edits bypassing safety checks
    #[arg(long = "force")]
    pub force: bool,
    /// Execute versioned edit plan
    #[arg(long = "edit-plan", value_name = "PLAN_JSON")]
    pub edit_plan: Option<String>,
    /// Execute JSON edit payload
    #[arg(long = "edit-payload", value_name = "PAYLOAD_JSON")]
    pub edit_payload: Option<String>,
}
impl TopSafetyArgs {
    pub fn is_active(&self) -> bool {
        self.test_gate.is_some()
            || self.situ.is_some()
            || self.safe_delete.is_some()
            || self.exercises.is_some()
            || self.seams
            || self.pr_context.is_some()
            || self.merge_scout.is_some()
            || self.dmm.is_some()
            || self.plan_lanes.is_some()
            || self.scan_skills.is_some()
            || self.edit_plan.is_some()
            || self.edit_payload.is_some()
            || self.partition.is_some()
            || self.pin_census
            || self.baseline
            || self.baseline_update
    }
}

/// Top-level architecture and discovery flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopArchitectureArgs {
    /// Internal functions with zero incoming callers across the repository
    #[arg(long = "dead-code")]
    pub dead_code: bool,
    /// Cluster call graph into cohesive modules (Louvain community detection)
    #[arg(long)]
    pub communities: bool,
    /// Inspect members and bridges of one specific community module
    #[arg(long, value_name = "ID")]
    pub community: Option<u32>,
    /// File-to-file dependency graph with instability and cycle metrics
    #[arg(long)]
    pub deps: bool,
    /// Ego-graph around a symbol
    #[arg(long, value_name = "SYM")]
    pub around: Option<String>,
    /// Depth for --around ego-graph (default: 1)
    #[arg(long = "around-depth", default_value_t = 1)]
    pub around_depth: u32,
    /// Fanout limit for --around ego-graph (default: 32)
    #[arg(long = "around-fanout", default_value_t = 32)]
    pub around_fanout: usize,
    /// Names used but never defined in the index (external stdlib/deps surface)
    #[arg(long = "external-surface")]
    pub external_surface: bool,
    /// Enforce architectural layering rules from file (exits 2 on violation)
    #[arg(long = "arch", value_name = "RULES_FILE")]
    pub arch: Option<String>,
    /// Emit Mermaid flowchart diagram of module call graph
    #[arg(long = "mermaid")]
    pub mermaid: bool,
    /// Multi-level hierarchical Louvain community clustering
    #[arg(long = "zoom", value_name = "DEPTH")]
    pub zoom: Option<Option<u32>>,
    /// Max depth levels for --zoom
    #[arg(long = "zoom-levels", value_name = "N")]
    pub zoom_levels: Option<u32>,
    /// Include built-in functions in external-surface
    #[arg(long = "include-builtins")]
    pub include_builtins: bool,
    /// SARIF format export for lint
    #[arg(long = "sarif")]
    pub sarif: bool,
    /// Annotate every symbol with fan-in, fan-out, complexity, and change-amplification
    #[arg(long = "metrics")]
    pub metrics: bool,
    /// Entire quality panel in one report (strict|default|lenient)
    #[arg(long = "quality-panel", num_args = 0..=1, default_missing_value = "default", value_name = "PRESET")]
    pub quality_panel: Option<Option<String>>,
    /// Show one struct's memory layout with computed field offsets and padding
    #[arg(long = "layout", value_name = "STRUCT")]
    pub layout: Option<String>,
    /// Cache-conscious field affinity analysis
    #[arg(long = "field-affinity", num_args = 0..=1, default_missing_value = "", value_name = "STRUCT")]
    pub field_affinity: Option<Option<String>>,
    /// Local-reasoning lens measuring share of context outside a unit's file
    #[arg(long = "context-ratio")]
    pub context_ratio: bool,
    /// Four orthogonal evidence families joined, ranked by distinct families fired
    #[arg(long = "ensemble")]
    pub ensemble: bool,
    /// Comment-to-identifier coherence check
    #[arg(long = "comment-coherence")]
    pub comment_coherence: bool,
}
impl TopArchitectureArgs {
    pub fn is_active(&self) -> bool {
        self.dead_code
            || self.communities
            || self.community.is_some()
            || self.deps
            || self.around.is_some()
            || self.external_surface
            || self.arch.is_some()
            || self.mermaid
            || self.zoom.is_some()
            || self.metrics
            || self.quality_panel.is_some()
            || self.layout.is_some()
            || self.field_affinity.is_some()
            || self.context_ratio
            || self.ensemble
            || self.comment_coherence
    }
}

/// Top-level global modifiers and batching flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopModifierArgs {
    /// Emit JSON instead of XML
    #[arg(long)]
    pub json: bool,
    /// Paginate: maximum number of items to emit
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
    /// Paginate: number of items to skip
    #[arg(long, value_name = "M")]
    pub offset: Option<usize>,
    /// Emit ordering: important-first (default), important-last, or stable
    #[arg(long, default_value = "important-first")]
    pub order: String,
    /// Run several lookups in one turn from newline-delimited verb:arg file
    #[arg(long, value_name = "FILE")]
    pub batch: Option<String>,
    /// Explicit cache file path
    #[arg(long, value_name = "PATH")]
    pub cache: Option<String>,
    /// Disable on-disk binary auto-caching (force cold parse)
    #[arg(long = "no-cache")]
    pub no_cache: bool,
    /// Select output format (e.g. candidates, columnar, json)
    #[arg(long = "format", value_name = "FORMAT")]
    pub format: Option<String>,
    /// Exclude glob or directory pattern from indexing
    #[arg(long = "exclude", value_name = "GLOB")]
    pub exclude: Option<String>,
    /// Do not respect .gitignore rules during crawling
    #[arg(long = "no-ignore")]
    pub no_ignore: bool,
    /// Exclude test files and directories from analysis
    #[arg(long = "ignore-tests")]
    pub ignore_tests: bool,
    /// Maximum file size in bytes to index
    #[arg(long = "max-file-size", value_name = "BYTES")]
    pub max_file_size: Option<u64>,
    /// Disable secret redaction on emitted snippets
    #[arg(long = "no-redact")]
    pub no_redact: bool,
    /// Disable stable dependency violation section in --deps
    #[arg(long = "no-stable")]
    pub no_stable: bool,
    /// Drop unranked <inv> file inventory tier from map and metrics output
    #[arg(long = "no-inventory")]
    pub no_inventory: bool,
    /// Explicit RSS memory guard ceiling (e.g. 512M, 2G)
    #[arg(long = "max-memory", value_name = "N[K|M|G]")]
    pub max_memory: Option<String>,
    /// Compress output using compact token dialect
    #[arg(long = "compress")]
    pub compress: bool,
    /// Export full index to file
    #[arg(long = "export", value_name = "FILE")]
    pub export: Option<String>,
    /// Write binary index cache to file
    #[arg(long = "index-out", value_name = "FILE")]
    pub index_out: Option<String>,
    /// Enable LSP server bridge
    #[arg(long = "lsp")]
    pub lsp: bool,
    /// Require authentication token for MCP transport
    #[arg(long = "mcp-token", value_name = "TOKEN")]
    pub mcp_token: Option<String>,
    /// Legend posture for stdio MCP server (session|inline)
    #[arg(long = "mcp-legend", value_name = "MODE")]
    pub mcp_legend: Option<String>,
    /// Restrict exposed MCP tools to comma-separated list or profile (core|full)
    #[arg(long = "mcp-tools", value_name = "LIST")]
    pub mcp_tools: Option<String>,
    /// Target agent profile for output shaping
    #[arg(long = "agent", value_name = "AGENT")]
    pub agent: Option<String>,
    /// Include call-graph edges in task/retrieval output
    #[arg(long = "with-graph")]
    pub with_graph: bool,
    /// Annotate symbols with runtime profile weights
    #[arg(long = "with-profile", value_name = "FILE")]
    pub with_profile: Option<String>,
    /// Execute runtime trace command and map onto symbols
    #[arg(long = "run-trace", value_name = "CMD")]
    pub run_trace: Option<String>,
    /// Timeout in seconds for --run-trace execution
    #[arg(long = "run-timeout", value_name = "SEC")]
    pub run_timeout: Option<u64>,
    /// Run co-change recall vs lexical rankers self-evaluation
    #[arg(long = "eval")]
    pub eval: bool,
    /// Known-item retrieval benchmark (MRR, recall@1/5/10)
    #[arg(long = "eval-retrieval")]
    pub eval_retrieval: bool,
    /// Mined git commit benchmark evaluation
    #[arg(long = "eval-mined")]
    pub eval_mined: bool,
    /// Agent skill effectiveness evaluation
    #[arg(long = "eval-skills")]
    pub eval_skills: bool,
    /// Cross-branch stray content evaluation
    #[arg(long = "eval-stray")]
    pub eval_stray: bool,
    /// Run daemon listener
    #[arg(long = "listen")]
    pub listen: bool,
    /// Pretty-print indented XML (default is minified token-optimized XML)
    #[arg(long = "pretty")]
    pub pretty: bool,
}
impl TopModifierArgs {
    pub fn is_pretty(&self) -> bool {
        self.pretty || self.format.as_deref() == Some("pretty")
    }

    pub fn is_json(&self) -> bool {
        self.json || self.format.as_deref() == Some("json")
    }
}

/// Top-level retrieval & architecture flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopRetrievalArgs {
    /// Importance-weighted detail: full bodies for top N symbols, or lift collapsed rows
    #[arg(long = "detail", num_args = 0..=1, default_missing_value = "1", value_name = "N")]
    pub detail: Option<usize>,
    /// Cut returned set at the relevance cliff instead of fixed top-k
    #[arg(long = "adaptive")]
    pub adaptive: bool,
    /// Let --doc-drift and --whereis consult git history for deleted names
    #[arg(long = "with-history")]
    pub with_history: bool,
    /// Skip query-shape router and force plain subtoken+body BM25
    #[arg(long = "no-route")]
    pub no_route: bool,
    /// Disable query-mention anchor score lifting
    #[arg(long = "no-mention-boost")]
    pub no_mention_boost: bool,
    /// Disable doc-mention backtick surfacing
    #[arg(long = "no-doc-mention")]
    pub no_doc_mention: bool,
    /// Restore automatic body walk on the conceptual route
    #[arg(long = "auto-bodies")]
    pub auto_bodies: bool,
    /// Opt in to full lego and compose sections (lego,compose)
    #[arg(long = "sections", value_name = "SECTIONS")]
    pub sections: Option<String>,
    #[arg(long = "pack-signatures")]
    pub pack_signatures: bool,
    #[arg(long = "for", visible_alias = "query", value_name = "TASK")]
    pub task_for: Option<String>,
    /// Literal trigram text search
    #[arg(long, value_name = "PAT")]
    pub grep: Option<String>,
    /// Restrict grep to files matching path substring
    #[arg(long = "grep-in", value_name = "DIR")]
    pub grep_in: Option<String>,
    /// Restrict grep to specific symbol scope
    #[arg(long = "grep-scope", value_name = "SCOPE")]
    pub grep_scope: Option<String>,
    /// Conjunction filter for structural search
    #[arg(long = "and", value_name = "PAT")]
    pub and_pat: Option<String>,
    /// Negation filter for structural search
    #[arg(long = "not", value_name = "PAT")]
    pub not_pat: Option<String>,
    /// Enclosing symbol or directory filter
    #[arg(long = "in", value_name = "SCOPE")]
    pub in_scope: Option<String>,
    /// Emit compact refetch handles on retrieval rows
    #[arg(long = "handles")]
    pub handles: bool,
    /// Expand a compact handle returned by --handles
    #[arg(long = "refetch", value_name = "HANDLE")]
    pub refetch: Option<String>,
    /// Recommend the best ripwire verb for a natural-language task
    #[arg(long = "help-task", value_name = "TASK")]
    pub help_task: Option<String>,
    /// Minimal connecting subgraph relating symbols (A,B,...)
    #[arg(long, value_name = "A,B")]
    pub connect: Option<String>,
    /// Shortest directed call path between two symbols (A:B or A,B)
    #[arg(long = "path-between", visible_alias = "path", value_name = "A:B")]
    pub path_between: Option<String>,
    /// Retrieve best-in-class existing instance of code to imitate
    #[arg(long, value_name = "TASK|KIND")]
    pub exemplar: Option<String>,
    /// Show one interface and every type implementing it
    #[arg(long, value_name = "TYPE")]
    pub lego: Option<String>,
    /// Search plans, designs, and docs for relevant notes
    #[arg(long, value_name = "TASK")]
    pub recall: Option<String>,
    /// Map a stack trace, sanitizer report or compiler error onto indexed symbols
    #[arg(long = "from-trace", value_name = "FILE")]
    pub from_trace: Option<String>,
    /// Resolve innermost symbol chain enclosing FILE:LINE
    #[arg(long = "at", value_name = "FILE:LINE")]
    pub at: Option<String>,
    /// Formally verify a code relationship claim (e.g. calls(A,B))
    #[arg(long = "verify", value_name = "CLAIM")]
    pub verify: Option<String>,
    /// Package single-call orientation task bundle under token budget
    #[arg(long = "pack-task", value_name = "TASK")]
    pub pack_task: Option<String>,
    /// Evaluate small functional graph query DSL expression
    #[arg(long = "graph-query", value_name = "EXPR")]
    pub graph_query: Option<String>,
    /// AST pattern query expression
    #[arg(long = "match", value_name = "QUERY")]
    pub ast_match: Option<String>,
    /// Code-shaped structural pattern search
    #[arg(long = "pattern", value_name = "PAT")]
    pub pattern: Option<String>,
    /// Regular expression search
    #[arg(long = "regex", value_name = "PAT")]
    pub regex: Option<String>,
    /// Emit only signatures without bodies
    #[arg(long = "signatures-only")]
    pub signatures_only: bool,
    /// Connecting radius for --connect
    #[arg(long = "connect-radius", default_value_t = 2)]
    pub connect_radius: u32,
    /// Lines before grep hit
    #[arg(long = "grep-before", value_name = "N")]
    pub grep_before: Option<usize>,
    /// Lines after grep hit
    #[arg(long = "grep-after", value_name = "N")]
    pub grep_after: Option<usize>,
    /// Lines around grep hit
    #[arg(long = "grep-context", value_name = "N")]
    pub grep_context: Option<usize>,
}
impl TopRetrievalArgs {
    pub fn is_active(&self) -> bool {
        let opts = [
            &self.task_for,
            &self.grep,
            &self.connect,
            &self.path_between,
            &self.exemplar,
            &self.lego,
            &self.recall,
            &self.from_trace,
            &self.at,
            &self.verify,
            &self.pack_task,
            &self.graph_query,
            &self.ast_match,
            &self.pattern,
            &self.regex,
            &self.help_task,
            &self.refetch,
        ];
        opts.iter().any(|o| o.is_some()) || self.pack_signatures
    }
}

/// Top-level workspace & audit flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopWorkspaceArgs {
    /// Find which branches or trees define a symbol
    #[arg(long, value_name = "SYM")]
    pub whereis: Option<String>,
    /// Emit verbatim matched lines in --whereis without length truncation
    #[arg(long = "whereis-full-text")]
    pub whereis_full_text: bool,
    /// Select --whereis listing filter (defs|refs|all)
    #[arg(long = "whereis-listing", value_name = "MODE")]
    pub whereis_listing: Option<String>,
    /// Authorship and bus-factor analysis
    #[arg(long)]
    pub owners: bool,
    /// Detect stale doc references
    #[arg(long = "doc-drift")]
    pub doc_drift: bool,
    /// Scan dark feature flags and environment reads
    #[arg(long)]
    pub flags: bool,
    /// Divergent branch content tracking
    #[arg(long = "stray-content")]
    pub stray_content: bool,
    /// Markdown docs referencing symbol in backticks
    #[arg(long, value_name = "SYM")]
    pub mentions: Option<String>,
    /// Environment and dependency health diagnostics
    #[arg(long)]
    pub doctor: bool,
    /// Continuation packet for interrupted agent sessions
    #[arg(long)]
    pub handoff: bool,
    /// View pinned session gotchas and invariants (.ripwire_notes)
    #[arg(long)]
    pub notes: bool,
    /// Pin gotcha note to .ripwire_notes (e.g. --note-add="SYM: text")
    #[arg(long = "note-add", value_name = "NOTE")]
    pub note_add: Option<String>,
    /// Scan blast radius of toggling a feature flag
    #[arg(long = "flip", value_name = "GATE")]
    pub flip: Option<String>,
    /// Gateability check for doc-drift
    #[arg(long = "gateability")]
    pub gateability: bool,
    /// Check ABI byte layout across branches in stray-content
    #[arg(long = "abi")]
    pub abi: bool,
    /// Generate landing plan in stray-content
    #[arg(long = "plan")]
    pub plan: bool,
    /// Lint structure of a PLAN file
    #[arg(long = "plan-lint", value_name = "FILE")]
    pub plan_lint: Option<String>,
    /// List files skipped due to size limits
    #[arg(long = "skipped")]
    pub skipped: bool,
    /// Replace symbol definition body with text
    #[arg(long = "replace-symbol-body", value_name = "SYM:BODY")]
    pub replace_symbol_body: Option<String>,
    /// Insert text immediately before a symbol definition
    #[arg(long = "insert-before-symbol", value_name = "SYM:TEXT")]
    pub insert_before_symbol: Option<String>,
    /// Insert text immediately after a symbol definition
    #[arg(long = "insert-after-symbol", value_name = "SYM:TEXT")]
    pub insert_after_symbol: Option<String>,
}

impl TopWorkspaceArgs {
    #[must_use]
    pub fn active_edit_op(&self) -> Option<(&str, crate::mcp::edit::EditOp)> {
        self.replace_symbol_body
            .as_deref()
            .map(|s| (s, crate::mcp::edit::EditOp::ReplaceBody))
            .or_else(|| {
                self.insert_before_symbol
                    .as_deref()
                    .map(|s| (s, crate::mcp::edit::EditOp::InsertBefore))
            })
            .or_else(|| {
                self.insert_after_symbol
                    .as_deref()
                    .map(|s| (s, crate::mcp::edit::EditOp::InsertAfter))
            })
    }

    pub fn is_active(&self) -> bool {
        self.whereis.is_some()
            || self.owners
            || self.doc_drift
            || self.flags
            || self.stray_content
            || self.mentions.is_some()
            || self.doctor
            || self.handoff
            || self.notes
            || self.note_add.is_some()
            || self.flip.is_some()
            || self.plan_lint.is_some()
            || self.skipped
            || self.active_edit_op().is_some()
    }
}
/// Categorized workflow command groups.
#[derive(Subcommand, Debug, PartialEq)]
pub enum Commands {
    /// Exploration and task-based context orientation
    Orient(OrientArgs),
    /// Code navigation and call graph traversals (callers, callees, impact)
    Navigate(NavigateArgs),
    /// Pre-commit verification gates (edit-check, affected, quality-delta)
    Gate(GateArgs),
    /// Code complexity, churn, and architectural health metrics
    Metrics(MetricsArgs),
    /// ARISE intra-procedural def-use variable slicing
    Slice(SliceArgs),
    /// Launch stdio JSON-RPC Model Context Protocol server
    Mcp(McpArgs),
    /// Manage and install ripwire-rs agent skills
    Skills(SkillsArgs),
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct OrientArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,
    /// Task description for relevance-ranked orientation
    #[arg(long = "for", value_name = "TASK")]
    pub for_task: Option<String>,
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct NavigateArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,
    /// Target symbol name
    #[arg(short, long)]
    pub symbol: Option<String>,
    /// Show direct 1-hop callers
    #[arg(long)]
    pub callers: bool,
    /// Show direct 1-hop callees
    #[arg(long)]
    pub callees: bool,
    /// Show transitive blast radius
    #[arg(long)]
    pub impact: bool,
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct GateArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,
    /// Verify 1-hop caller compatibility against git HEAD
    #[arg(long)]
    pub edit_check: Option<String>,
    /// Pin quality baseline stamped with git HEAD
    #[arg(long)]
    pub quality_baseline: bool,
    /// Measure quality delta against baseline
    #[arg(long)]
    pub quality_delta: bool,
    /// Show test files affected by changes
    #[arg(long)]
    pub affected: Option<String>,
    /// Detect AST code duplication and clones (Type-1, Type-2, Type-3)
    #[arg(long)]
    pub clones: bool,
    /// Minimum token length for clone detection
    #[arg(long, default_value_t = 40)]
    pub min_tokens: usize,
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct MetricsArgs {
    /// Target repository path
    #[arg(value_name = "PATH")]
    pub path: Option<String>,
    /// Show top maintenance hotspots (churn * ccx)
    #[arg(long)]
    pub hotspots: bool,
    /// Show file co-change coupling (Fowler's Shotgun Surgery)
    #[arg(long)]
    pub cochange: Option<Option<String>>,
    /// Limit git history traversal to commits after this date or revision
    #[arg(long)]
    pub since: Option<String>,
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct SliceArgs {
    /// Target variable criterion in symbol (e.g. sym:var)
    #[arg(value_name = "TARGET")]
    pub target: Option<String>,
    /// Dataflow direction (fwd, back, both)
    #[arg(long, default_value = "fwd")]
    pub flow: String,
}

#[derive(Args, Debug, Default, PartialEq)]
pub struct McpArgs {
    /// Target workspace root to pin
    #[arg(value_name = "PATH")]
    pub path: Option<String>,
}
#[derive(Args, Debug, Default, PartialEq)]
pub struct SkillsArgs {
    /// Target directory (default: ~/.agents/skills)
    #[arg(value_name = "DIR")]
    pub path: Option<String>,
}

/// Configuration for `--around` ego-graph queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AroundConfig {
    pub depth: u32,
    pub fanout: usize,
}

/// Capability trait for extracting pagination windows (`--limit`, `--offset`) from CLI modifiers.
pub trait Paginated {
    fn limit_opt(&self) -> Option<usize>;
    fn offset_opt(&self) -> Option<usize>;

    fn page_window(&self, total: usize, default_cap: usize) -> crate::serialize::PageWindow {
        crate::serialize::PageWindow::compute(
            total,
            default_cap,
            self.limit_opt(),
            self.offset_opt(),
        )
    }

    fn paginate_slice<'a, T>(
        &self,
        items: &'a [T],
        default_cap: usize,
    ) -> (crate::serialize::PageWindow, &'a [T])
    where
        Self: Sized,
    {
        use crate::serialize::PaginateSlice;
        items.paginate(default_cap, self.limit_opt(), self.offset_opt())
    }

    fn apply_pagination_to_json(&self, val: &mut serde_json::Value) {
        if let Some(lim) = self.limit_opt() {
            val["limit"] = serde_json::Value::from(lim as u64);
        }
        if let Some(off) = self.offset_opt() {
            val["offset"] = serde_json::Value::from(off as u64);
        }
    }
}

/// Capability trait for extracting token, byte, top-k, and detail budgets from CLI options.
pub trait Budgeted {
    fn pack_budget_opt(&self) -> Option<usize>;
    fn token_budget_opt(&self) -> Option<usize>;
    fn max_tokens_opt(&self) -> Option<usize>;
    fn top_k_opt(&self) -> Option<usize>;
    fn detail_opt(&self) -> Option<usize>;

    fn effective_budget_bytes(&self) -> Option<usize> {
        if self.top_k_opt().is_some()
            && self.pack_budget_opt().is_none()
            && self.token_budget_opt().is_none()
            && self.max_tokens_opt().is_none()
        {
            None
        } else {
            super::dispatch::compute_effective_budget(
                self.pack_budget_opt(),
                self.token_budget_opt(),
                self.max_tokens_opt(),
            )
        }
    }
}

/// Capability trait for git history scope modifiers (`--with-history`, `--since`).
pub trait HistoryScoped {
    fn with_history_flag(&self) -> bool;
    fn since_scope_opt(&self) -> Option<&str>;
}

/// Capability trait for output dialect selection (`--json`, `--pretty`, `--legend`).
pub trait OutputDialect {
    fn is_json_out(&self) -> bool;
    fn is_pretty_out(&self) -> bool;
    fn is_legend_none(&self) -> bool;
    fn target_root(&self) -> &str;

    fn wrap_xml_writer<'a, W: std::io::Write>(
        &self,
        inner: &'a mut W,
        force_plain: bool,
    ) -> crate::serialize::XmlWriter<'a, W> {
        crate::serialize::XmlWriter::with_options(
            inner,
            self.is_pretty_out() || force_plain,
            self.is_json_out() && !force_plain,
            self.is_legend_none() && !force_plain,
        )
    }
}
impl Paginated for TopModifierArgs {
    fn limit_opt(&self) -> Option<usize> {
        self.limit
    }
    fn offset_opt(&self) -> Option<usize> {
        self.offset
    }
}

impl Paginated for Cli {
    fn limit_opt(&self) -> Option<usize> {
        self.modifiers.limit
    }
    fn offset_opt(&self) -> Option<usize> {
        self.modifiers.offset
    }
}

impl Budgeted for Cli {
    fn pack_budget_opt(&self) -> Option<usize> {
        self.pack_budget
    }
    fn token_budget_opt(&self) -> Option<usize> {
        self.token_budget
    }
    fn max_tokens_opt(&self) -> Option<usize> {
        self.max_tokens
    }
    fn top_k_opt(&self) -> Option<usize> {
        self.top_k
    }
    fn detail_opt(&self) -> Option<usize> {
        self.retrieval.detail
    }
}

impl HistoryScoped for Cli {
    fn with_history_flag(&self) -> bool {
        self.retrieval.with_history
    }
    fn since_scope_opt(&self) -> Option<&str> {
        self.since.as_deref()
    }
}

impl OutputDialect for Cli {
    fn is_json_out(&self) -> bool {
        self.modifiers.is_json()
    }
    fn is_pretty_out(&self) -> bool {
        self.modifiers.is_pretty()
    }
    fn is_legend_none(&self) -> bool {
        self.legend.eq_ignore_ascii_case("none")
    }
    fn target_root(&self) -> &str {
        self.path.as_deref().unwrap_or(".")
    }
}

/// Trait implemented by each CLI argument group (`TopNavArgs`, `TopRetrievalArgs`, etc.).
pub trait VerbGroup {
    fn is_group_active(&self) -> bool;
}

impl TopNavArgs {
    #[must_use]
    pub fn active_query(&self) -> Option<(&str, super::queries::NavMode)> {
        use super::queries::NavMode;
        self.callers
            .as_deref()
            .map(|s| (s, NavMode::Callers))
            .or_else(|| self.callees.as_deref().map(|s| (s, NavMode::Callees)))
            .or_else(|| self.impact.as_deref().map(|s| (s, NavMode::Impact)))
            .or_else(|| self.uses.as_deref().map(|s| (s, NavMode::Uses)))
            .or_else(|| {
                self.find_symbol
                    .as_deref()
                    .map(|s| (s, NavMode::FindSymbol))
            })
    }
}

impl VerbGroup for TopNavArgs {
    fn is_group_active(&self) -> bool {
        self.active_query().is_some()
    }
}

impl VerbGroup for TopDetailArgs {
    fn is_group_active(&self) -> bool {
        self.is_active()
    }
}

impl VerbGroup for TopSafetyArgs {
    fn is_group_active(&self) -> bool {
        self.is_active()
    }
}

impl VerbGroup for TopArchitectureArgs {
    fn is_group_active(&self) -> bool {
        self.is_active()
    }
}

impl VerbGroup for TopRetrievalArgs {
    fn is_group_active(&self) -> bool {
        self.is_active()
    }
}

impl VerbGroup for TopWorkspaceArgs {
    fn is_group_active(&self) -> bool {
        self.is_active()
    }
}
