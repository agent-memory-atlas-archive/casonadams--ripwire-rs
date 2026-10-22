use clap::{Args, Parser, Subcommand};

/// Root command description.
pub const ABOUT: &str = "High-throughput agent context router and static analysis engine";

/// CLI command line options.
#[derive(Parser, Debug, Default, PartialEq)]
#[command(name = "ripwire-rs", version, about = ABOUT)]
pub struct Cli {
    /// Target repository path (default: current directory)
    #[arg(value_name = "PATH")]
    pub path: Option<String>,
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
    /// Workflow subcommand group, parsed by the command plugins
    #[arg(skip)]
    pub command: Option<Commands>,
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

    /// The subcommand the command line selected, with its own matches.
    #[must_use]
    pub fn subcommand(&self) -> Option<(&str, &clap::ArgMatches)> {
        self.0.as_ref()?.subcommand()
    }
}

/// Trait implemented by each CLI argument group.
pub trait VerbGroup {
    fn is_group_active(&self) -> bool;
}
