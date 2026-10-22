use ripwire_rs::commands::for_task::{self, ForTaskArgs};
use ripwire_rs::query::commands::QueryContext;
use ripwire_rs::serialize::PageRequest;
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, PageRankConfig, ingest_in_memory_files};

fn refresh_source() -> String {
    (0..12)
        .map(|i| format!("pub fn refresh_session_{i}() -> u32 {{\n    {i}\n}}\n"))
        .collect()
}

fn page(ctx: &QueryContext<'_>, offset: usize) -> String {
    let mut out = Vec::new();
    let page = PageRequest {
        limit: Some(5),
        offset: Some(offset),
        budget: None,
    };
    for_task::execute(
        ctx,
        "refresh session",
        &ForTaskArgs::default(),
        None,
        None,
        page,
        &mut out,
    )
    .unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn test_for_pages_partition_the_ranked_symbols_without_overlap() {
    let source = refresh_source();
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let files = [("src/session.rs", Lang::Rust, source.as_bytes())];
    let ingest = ingest_in_memory_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&PageRankConfig::default());
    let sources = [("src/session.rs", source.as_bytes())];
    let ctx = QueryContext {
        root: ":memory:",
        ingest: &ingest,
        graph: &graph,
        pr: &pr,
        interner: &interner,
        sources: &sources,
        commits: None,
        at: None,
    };

    let first = page(&ctx, 0);
    let doc = roxmltree::Document::parse(&first).unwrap();
    let sigs = doc.descendants().find(|n| n.has_tag_name("sigs")).unwrap();
    let total: usize = sigs.attribute("total").unwrap().parse().unwrap();
    assert!(total >= 10, "{first}");
    assert_eq!(sigs.attribute("shown"), Some("5"));
    assert_eq!(sigs.attribute("capped"), Some("1"));
    assert_eq!(sigs.attribute("next_offset"), Some("5"));

    let mut ranks = Vec::new();
    for offset in (0..total).step_by(5) {
        let xml = page(&ctx, offset);
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let sigs = doc.descendants().find(|n| n.has_tag_name("sigs")).unwrap();
        assert_eq!(
            sigs.attribute("total").map(str::parse::<usize>),
            Some(Ok(total))
        );
        ranks.extend(
            doc.descendants()
                .filter(|n| n.has_tag_name("d"))
                .map(|n| n.attribute("r").unwrap().parse::<usize>().unwrap()),
        );
    }
    assert_eq!(ranks, (1..=total).collect::<Vec<_>>());
}

#[test]
fn test_for_last_page_is_whole_and_says_so() {
    let source = refresh_source();
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let files = [("src/session.rs", Lang::Rust, source.as_bytes())];
    let ingest = ingest_in_memory_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let pr = graph.page_rank(&PageRankConfig::default());
    let sources = [("src/session.rs", source.as_bytes())];
    let ctx = QueryContext {
        root: ":memory:",
        ingest: &ingest,
        graph: &graph,
        pr: &pr,
        interner: &interner,
        sources: &sources,
        commits: None,
        at: None,
    };
    let first = page(&ctx, 0);
    let total: usize = roxmltree::Document::parse(&first)
        .unwrap()
        .descendants()
        .find(|n| n.has_tag_name("sigs"))
        .and_then(|n| n.attribute("total"))
        .unwrap()
        .parse()
        .unwrap();
    let past_end = page(&ctx, total + 3);
    let doc = roxmltree::Document::parse(&past_end).unwrap();
    let sigs = doc.descendants().find(|n| n.has_tag_name("sigs")).unwrap();
    assert_eq!(sigs.attribute("shown"), Some("0"));
    assert_eq!(sigs.attribute("has_more"), Some("0"));
    assert_eq!(sigs.attribute("next"), None);
}
