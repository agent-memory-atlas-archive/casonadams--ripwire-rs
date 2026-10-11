use rwire::{crawl_directory, ingest_crawled_files, CrawlerOptions, Interner, LanguageRegistry};
use std::env;
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    let target_dir = args.get(1).map(|s| s.as_str()).unwrap_or(".");
    let root = Path::new(target_dir);

    let start = Instant::now();
    let options = CrawlerOptions::default();
    let crawled_files = crawl_directory(root, &options);

    let registry = LanguageRegistry::new_tier1().expect("failed to initialize language registry");
    let interner = Interner::new();
    let ingest = ingest_crawled_files(&crawled_files, &registry, &interner);
    let elapsed = start.elapsed();

    println!(
        "rwire v0.1.0 — crawled {} files, indexed {} symbols, {} references in {:.2?}",
        ingest.file_count(),
        ingest.symbol_count(),
        ingest.reference_count(),
        elapsed
    );
}
