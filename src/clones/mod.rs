pub mod detector;
pub mod token_stream;

pub use detector::*;
pub use token_stream::*;

use crate::domain::Interner;
use crate::ingest::LanguageRegistry;
use std::io;

/// Default minimum token length for clone grouping.
pub const DEFAULT_MIN_CLONE_TOKENS: usize = 40;

/// Execute clone detection on the target repository and print results.
pub fn execute_clones_command(root: &str, min_tokens: usize) -> io::Result<()> {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().map_err(|e| io::Error::other(e.to_string()))?;

    let files = crate::crawler::crawl_directory(
        std::path::Path::new(root),
        &crate::crawler::CrawlerOptions::default(),
    );
    let ingest = crate::ingest::ingest_crawled_files(&files, &registry, &interner);
    let clones = find_clones_in_root(
        &ingest,
        &interner,
        min_tokens,
        None,
        Some(std::path::Path::new(root)),
    );
    let xml = render_clones_xml(&clones, &ingest, &interner);
    let mut stdout_lock = io::stdout().lock();
    let mut out = crate::serialize::XmlWriter::new(&mut stdout_lock, false);
    use std::io::Write;
    out.write_all(xml.as_bytes())?;
    out.flush()
}
