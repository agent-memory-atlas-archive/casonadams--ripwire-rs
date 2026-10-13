use clap::{Args, Parser, Subcommand};

/// CLI command line options.
#[derive(Parser, Debug, Default, PartialEq)]
#[command(
    name = "rw",
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
    /// Export interactive HTML call graph visualization to file
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
    /// Outline control-flow skeletons of symbols (SYM,SYM...)
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
}

/// Top-level retrieval & architecture flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopRetrievalArgs {
    /// Task-lens relevance ranking
    #[arg(long = "for", value_name = "TASK")]
    pub task_for: Option<String>,
    /// Literal trigram text search
    #[arg(long, value_name = "PAT")]
    pub grep: Option<String>,
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
        ];
        opts.iter().any(|o| o.is_some())
    }
}

/// Top-level workspace & audit flags.
#[derive(Args, Debug, Clone, Default, PartialEq)]
pub struct TopWorkspaceArgs {
    /// Find which branches or trees define a symbol
    #[arg(long, value_name = "SYM")]
    pub whereis: Option<String>,
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
            || self.replace_symbol_body.is_some()
            || self.insert_before_symbol.is_some()
            || self.insert_after_symbol.is_some()
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
