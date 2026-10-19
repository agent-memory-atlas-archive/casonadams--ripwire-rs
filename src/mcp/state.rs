use crate::domain::{IngestResult, Interner};
use crate::graph::Graph;
use crate::rank::{PageRankConfig, PageRankResult};

/// Server state caching the indexed workspace facts and call graph.
pub struct McpState {
    pub root: String,
    pub ingest: IngestResult,
    pub graph: Graph,
    pub ranks: Vec<f64>,
    pub interner: Interner,
    pub pr: PageRankResult,
    pub commits: Option<Vec<crate::git::history::CommitInfo>>,
    pub legend_sent: std::sync::atomic::AtomicBool,
}

impl McpState {
    fn load_with_cache(root: &str, no_cache: bool) -> Option<Self> {
        let (ingest, graph, pr, interner) =
            crate::cli::dispatch::load_repo_analysis_cached(root, None, no_cache).ok()?;
        let ranks = pr.ranks.clone();
        Some(Self {
            root: root.to_string(),
            ingest,
            graph,
            ranks,
            interner,
            pr,
            commits: None,
            legend_sent: std::sync::atomic::AtomicBool::new(false),
        })
    }

    #[must_use]
    pub fn load(root: &str) -> Option<Self> {
        Self::load_with_cache(root, false)
    }

    #[must_use]
    pub fn load_uncached(root: &str) -> Option<Self> {
        Self::load_with_cache(root, true)
    }
    #[must_use]
    pub fn from_ingest(root: &str, ingest: IngestResult, interner: Interner) -> Self {
        let graph = Graph::build(&ingest, &interner);
        let pr = graph.page_rank(&PageRankConfig::default());
        let ranks = pr.ranks.clone();
        Self {
            root: root.to_string(),
            ingest,
            graph,
            ranks,
            interner,
            pr,
            commits: None,
            legend_sent: std::sync::atomic::AtomicBool::new(false),
        }
    }

    #[must_use]
    pub fn with_commits(mut self, commits: Vec<crate::git::history::CommitInfo>) -> Self {
        self.commits = Some(commits);
        self
    }
}

impl<'a> crate::domain::CorpusView<'a> for &'a McpState {
    #[inline]
    fn ingest(&self) -> &'a IngestResult {
        &self.ingest
    }

    #[inline]
    fn interner(&self) -> &'a Interner {
        &self.interner
    }
}
