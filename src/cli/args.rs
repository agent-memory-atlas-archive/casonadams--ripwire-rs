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
    /// Ranking signal: pagerank, authority, hub, degree, ccx, stable
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
    /// Overlay SCIP compiler-backed index
    #[arg(long = "scip", global = true, value_name = "FILE")]
    pub scip: Option<String>,

    /// Parsed values of command-plugin flags
    #[arg(skip)]
    pub plugins: PluginArgs,
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
    /// Explicit per-lane brief file for --plan-lanes (one lane task per line)
    #[arg(long = "brief", num_args = 0..=1, default_missing_value = "", value_name = "FILE")]
    pub brief: Option<String>,
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
    #[arg(long = "no-cache", overrides_with = "no_cache")]
    pub no_cache: bool,
    /// Select output format (e.g. candidates, columnar, json)
    #[arg(long = "format", value_name = "FORMAT")]
    pub format: Option<String>,
    /// Exclude glob or directory pattern from indexing
    #[arg(long = "exclude", value_name = "GLOB")]
    pub exclude: Option<String>,
    /// Do not respect .gitignore rules during crawling
    #[arg(long = "no-ignore", overrides_with = "no_ignore")]
    pub no_ignore: bool,
    /// Exclude test files and directories from analysis
    #[arg(long = "ignore-tests", overrides_with = "ignore_tests")]
    pub ignore_tests: bool,
    /// Maximum file size in bytes to index
    #[arg(long = "max-file-size", value_name = "BYTES")]
    pub max_file_size: Option<u64>,
    /// Disable secret redaction on emitted snippets
    #[arg(long = "no-redact", overrides_with = "no_redact")]
    pub no_redact: bool,
    /// Emit deterministic path-ordered map without volatile rank scores
    #[arg(long = "stable", overrides_with = "stable")]
    pub stable: bool,
    /// Disable the stable dependency violation section in --deps
    #[arg(long = "no-stable", overrides_with = "no_stable")]
    pub no_stable: bool,
    /// Drop unranked <inv> file inventory tier from map and metrics output
    #[arg(long = "no-inventory", overrides_with = "no_inventory")]
    pub no_inventory: bool,
    /// Explicit RSS memory guard ceiling (e.g. 512M, 2G)
    #[arg(long = "max-memory", value_name = "N[K|M|G]")]
    pub max_memory: Option<String>,
    /// Compress output using compact token dialect
    #[arg(long = "compress", overrides_with = "compress")]
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
}

impl HistoryScoped for Cli {
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

/// Matches for the flags command plugins own; plugins decode their own typed `Args` from it.
#[derive(Debug, Clone, Default)]
pub struct PluginArgs(Option<clap::ArgMatches>);

impl PartialEq for PluginArgs {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl PluginArgs {
    #[must_use]
    pub fn new(matches: clap::ArgMatches) -> Self {
        Self(Some(matches))
    }

    /// The plugin's parsed flags, `None` when the command line was not parsed with that plugin.
    #[must_use]
    pub fn decode<A: clap::FromArgMatches>(&self) -> Option<A> {
        A::from_arg_matches(self.0.as_ref()?).ok()
    }
}

/// Trait implemented by each CLI argument group.
pub trait VerbGroup {
    fn is_group_active(&self) -> bool;
}
macro_rules! impl_verb_group {
    ($ty:ty, opts: [$($opt:ident),* $(,)?], flags: [$($flag:ident),* $(,)?] $(, extra: $extra:ident)?) => {
        impl $ty {
            #[must_use]
            pub fn is_active(&self) -> bool {
                $(self.$opt.is_some() ||)* $(self.$flag ||)* $(self.$extra().is_some() ||)? false
            }
        }
        impl VerbGroup for $ty {
            fn is_group_active(&self) -> bool {
                self.is_active()
            }
        }
    };
}

impl_verb_group!(
    TopSafetyArgs,
    opts: [
        test_gate, situ, safe_delete, exercises, pr_context, merge_scout,
        dmm, plan_lanes, scan_skills, edit_plan, edit_payload, partition
    ],
    flags: [seams, pin_census, baseline, baseline_update]
);

impl_verb_group!(
    TopArchitectureArgs,
    opts: [community, around, arch, zoom, quality_panel, layout, field_affinity],
    flags: [
        dead_code, communities, deps, external_surface, mermaid,
        metrics, context_ratio, ensemble, comment_coherence
    ]
);
