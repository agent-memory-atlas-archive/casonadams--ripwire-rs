use ripwire_rs::metrics::serialize_executive_report_md;
use ripwire_rs::query::safety::SafetyContext;
use ripwire_rs::serialize::PageRequest;
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, PageRankConfig, ingest_in_memory_files};

/// Four directories whose functions call across each other, and every one imports the shared helper.
fn corpus() -> Vec<(String, String)> {
    let mut files = vec![
        (
            "shared/core.ts".to_string(),
            "export function core_helper(): number {\n  return 1;\n}\n".to_string(),
        ),
        (
            "shared/format.ts".to_string(),
            "export function format_value(): number {\n  return 2;\n}\n".to_string(),
        ),
    ];
    for dir in ["alpha", "beta", "gamma", "delta"] {
        let next = match dir {
            "alpha" => "beta",
            "beta" => "gamma",
            "gamma" => "delta",
            _ => "alpha",
        };
        let body = format!(
            "import {{ core_helper }} from \"../shared/core\";\n\
             import {{ format_value }} from \"../shared/format\";\n\
             export function {dir}_entry(): number {{ return {dir}_step() + {next}_step() + core_helper() + format_value(); }}\n\
             function {dir}_step(): number {{ return core_helper(); }}\n\
             function {next}_step(): number {{ return core_helper(); }}\n"
        );
        files.push((format!("{dir}/mod.ts"), body));
    }
    files
}

fn report(page: PageRequest) -> String {
    let owned = corpus();
    let borrowed: Vec<(&str, Lang, &[u8])> = owned
        .iter()
        .map(|(path, text)| (path.as_str(), Lang::TypeScript, text.as_bytes()))
        .collect();
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(&borrowed, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let ranks = graph.page_rank(&PageRankConfig::default()).ranks;
    let facts = SafetyContext {
        root: ":memory:",
        ingest: &ingest,
        graph: &graph,
        interner: &interner,
    };
    let mut out = Vec::new();
    serialize_executive_report_md(&facts, &ranks, page, &mut out).unwrap();
    String::from_utf8(out).unwrap()
}

/// The `(showing S of T)` pair of the list under `heading`.
fn showing(text: &str, heading: &str) -> (usize, usize) {
    let line = text
        .lines()
        .find(|l| l.starts_with(&format!("## {heading} ")))
        .unwrap_or_else(|| panic!("no heading {heading}: {text}"));
    let inner = line
        .split("(showing ")
        .nth(1)
        .unwrap()
        .trim_end_matches(')');
    let mut parts = inner.split(" of ");
    let shown = parts.next().unwrap().parse().unwrap();
    let total = parts.next().unwrap().parse().unwrap();
    (shown, total)
}

/// The bullet rows under `heading`, up to the next heading.
fn rows(text: &str, heading: &str) -> Vec<String> {
    let mut lines = text
        .lines()
        .skip_while(|l| !l.starts_with(&format!("## {heading} ")));
    lines.next();
    lines
        .take_while(|l| !l.starts_with("## "))
        .filter(|l| l.starts_with("- ") || l.starts_with('|'))
        .filter(|l| {
            !l.starts_with("- next:") && !l.starts_with("|---") && !l.starts_with("| Symbol")
        })
        .map(str::to_string)
        .collect()
}

const LISTS: [&str; 4] = [
    "Modules (call-graph clusters)",
    "Cross-module bridges",
    "God files (most depended-on)",
    "Top Symbols by PageRank",
];

#[test]
fn test_report_lists_show_truthful_counts_and_a_next_route_when_capped() {
    let whole = report(PageRequest::default());
    for heading in LISTS {
        let (shown, total) = showing(&whole, heading);
        assert_eq!(shown, total, "{heading} is whole by default: {whole}");
        assert!(
            total >= 2,
            "{heading} needs several rows to window: {whole}"
        );
    }

    let capped = report(PageRequest {
        limit: Some(1),
        offset: None,
        budget: None,
    });
    for heading in LISTS {
        let (shown, total) = showing(&capped, heading);
        assert_eq!(shown, 1, "{heading}: {capped}");
        assert!(total >= 2, "{heading}: {capped}");
        let next = "--report --limit=1 --offset=1";
        assert!(
            capped.contains(next),
            "{heading}: next route missing: {capped}"
        );
    }
}

#[test]
fn test_report_limit_offset_pages_are_disjoint_and_complete() {
    let whole = report(PageRequest::default());
    for heading in LISTS {
        let (_, total) = showing(&whole, heading);
        let full = rows(&whole, heading);
        assert_eq!(full.len(), total, "{heading}: {whole}");

        let mut paged = Vec::new();
        for offset in (0..total).step_by(2) {
            let page = report(PageRequest {
                limit: Some(2),
                offset: Some(offset),
                budget: None,
            });
            let (shown, reported_total) = showing(&page, heading);
            assert_eq!(
                reported_total, total,
                "{heading} total is stable across pages"
            );
            assert_eq!(
                shown,
                (total - offset).min(2),
                "{heading} at offset {offset}"
            );
            paged.extend(rows(&page, heading));
        }
        assert_eq!(paged, full, "{heading}: pages overlap or omit rows");
    }
}
