use rwire::{
    crawl_directory, ingest_crawled_files, CrawlerOptions, Graph, Interner, LanguageRegistry,
};
use std::{env, path::Path, time::Instant};

fn main() {
    let target = env::args().nth(1).unwrap_or_else(|| ".".into());
    let start = Instant::now();

    let files = crawl_directory(Path::new(&target), &CrawlerOptions::default());
    let registry = LanguageRegistry::new_tier1().expect("failed to init language registry");
    let interner = Interner::new();
    let ingest = ingest_crawled_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);

    println!(
        "rwire v0.1.0 — crawled {} files, indexed {} symbols, {} references, resolved {} edges in {:.2?}",
        ingest.file_count(),
        ingest.symbol_count(),
        ingest.reference_count(),
        graph.edge_count(),
        start.elapsed()
    );
}
