use ripwire_rs::crawler::{
    CrawledFile, CrawlerOptions, DEFAULT_JSON_CEILING, DEFAULT_YAML_CEILING, evaluate_candidate,
    is_binary_buffer,
};
use ripwire_rs::domain::Lang;

#[test]
fn test_crawl_deterministic_and_denylist() {
    let candidates = [
        ("src/b.rs", 10, false),
        ("src/a.rs", 10, false),
        ("target/ign.rs", 10, false),
        (".git/config", 50, false),
        ("src/unknown.xyz", 10, false),
        ("binary.rs", 10, true),
        ("empty.rs", 0, false),
        ("Dockerfile", 20, false),
    ];

    let opts = CrawlerOptions::default();
    let mut files: Vec<CrawledFile> = candidates
        .iter()
        .filter_map(|&(rel_path, size, is_bin)| evaluate_candidate(rel_path, size, is_bin, &opts))
        .collect();

    files.sort_by(|a, b| a.rel_path.as_bytes().cmp(b.rel_path.as_bytes()));

    assert_eq!(files.len(), 3);
    assert_eq!(files[0].rel_path, "Dockerfile");
    assert_eq!(files[0].lang, Lang::Dockerfile);
    assert_eq!(files[1].rel_path, "src/a.rs");
    assert_eq!(files[2].rel_path, "src/b.rs");
    assert_eq!(files[1].lang, Lang::Rust);
    assert!(is_binary_buffer(b"\0hello"));
    assert!(!is_binary_buffer(b"hello world"));
}

#[test]
fn test_crawl_scope_ignore_tests_and_max_file_size() {
    let scoped = CrawlerOptions {
        max_file_size: 100,
        ignore_tests: true,
        ..CrawlerOptions::default()
    };
    let kept = |rel: &str, size: u64| evaluate_candidate(rel, size, false, &scoped).is_some();
    assert!(kept("src/lib.rs", 100));
    assert!(!kept("src/lib.rs", 101));
    assert!(!kept("tests/test_cli.rs", 10));
    assert!(!kept("src/foo_test.go", 10));
    let defaults = CrawlerOptions::default();
    assert!(evaluate_candidate("tests/test_cli.rs", 10, false, &defaults).is_some());
    assert!(evaluate_candidate("src/lib.rs", 101, false, &defaults).is_some());
    assert!(evaluate_candidate("data.json", DEFAULT_JSON_CEILING, false, &defaults).is_some());
    assert!(evaluate_candidate("data.json", DEFAULT_JSON_CEILING + 1, false, &defaults).is_none());
    assert!(
        evaluate_candidate("config.yaml", DEFAULT_YAML_CEILING + 1, false, &defaults).is_none()
    );
    assert!(evaluate_candidate(".github/workflows/ci.yml", 20, false, &defaults).is_some());
    assert!(evaluate_candidate(".eslintrc.json", 20, false, &defaults).is_some());
    assert!(evaluate_candidate(".vscode/settings.json", 20, false, &defaults).is_none());
    assert!(evaluate_candidate(".claude/settings.json", 20, false, &defaults).is_none());
    for generated in ["api/x.pb.go", "m_pb2.py", "web/app.min.js"] {
        assert!(
            evaluate_candidate(generated, 20, false, &defaults).is_none(),
            "{generated}"
        );
    }
    for kept in ["api/x.pb.ts", "y_gen.go", "web/app.min.mjs"] {
        assert!(
            evaluate_candidate(kept, 20, false, &defaults).is_some(),
            "{kept}"
        );
    }
}
