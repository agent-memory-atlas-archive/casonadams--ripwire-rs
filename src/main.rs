use rwire::{
    crawl_directory, ingest_crawled_files, serialize_xml, CrawlerOptions, Graph, Interner,
    LanguageRegistry, PageRankConfig, SerializeOptions,
};
use std::{env, io, path::Path};

fn main() -> io::Result<()> {
    let target = env::args().nth(1).unwrap_or_else(|| ".".into());
    let files = crawl_directory(Path::new(&target), &CrawlerOptions::default());
    let registry = LanguageRegistry::new_tier1().expect("failed to init language registry");
    let interner = Interner::new();
    let ingest = ingest_crawled_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&PageRankConfig::default());

    let mut stdout = io::stdout().lock();
    serialize_xml(
        &ingest,
        &graph,
        &pr,
        &interner,
        &mut stdout,
        &SerializeOptions {
            root: target,
            top_k: None,
            include_pr: true,
        },
    )
}
