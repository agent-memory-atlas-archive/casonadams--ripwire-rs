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
}

impl McpState {
    #[must_use]
    pub fn load(root: &str) -> Option<Self> {
        let (ingest, graph, pr, interner) =
            crate::cli::dispatch::load_repo_analysis_cached(root, None, false).ok()?;
        let ranks = pr.ranks.clone();
        Some(Self {
            root: root.to_string(),
            ingest,
            graph,
            ranks,
            interner,
            pr,
            commits: None,
        })
    }

    #[must_use]
    pub fn load_uncached(root: &str) -> Option<Self> {
        let (ingest, graph, pr, interner) =
            crate::cli::dispatch::load_repo_analysis_cached(root, None, true).ok()?;
        let ranks = pr.ranks.clone();
        Some(Self {
            root: root.to_string(),
            ingest,
            graph,
            ranks,
            interner,
            pr,
            commits: None,
        })
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
        }
    }

    #[must_use]
    pub fn with_commits(mut self, commits: Vec<crate::git::history::CommitInfo>) -> Self {
        self.commits = Some(commits);
        self
    }
}
