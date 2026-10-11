use rwire::{
    crawl_directory, ingest_crawled_files, CrawlerOptions, IngestResult, Interner,
    LanguageRegistry, RefRole, SymKind,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_ingest_tier1_fixtures() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let fixtures = [
        (
            "db.rs",
            "pub struct Database; impl Database { pub fn connect(&self) { log_info(); } } fn log_info() {}",
        ),
        (
            "server.ts",
            "export interface Config {} export class Server { start() { handleRequest(); } } function handleRequest() {}",
        ),
        (
            "router.js",
            "class Router { route() { dispatch(); } } function dispatch() {}",
        ),
        (
            "auth.py",
            "class AuthManager:\n    def verify(self):\n        check_token()\ndef check_token():\n    pass\n",
        ),
        (
            "service.go",
            "package main\ntype Service struct{}\nfunc (s *Service) Run() { helper() }\nfunc helper() {}\n",
        ),
    ];

    for (name, content) in fixtures {
        fs::write(root.join(name), content).unwrap();
    }

    let crawled = crawl_directory(root, &CrawlerOptions::default());
    assert_eq!(crawled.len(), 5);

    let rel_paths: Vec<&str> = crawled.iter().map(|f| f.rel_path.as_str()).collect();
    assert_eq!(
        rel_paths,
        ["auth.py", "db.rs", "router.js", "server.ts", "service.go"]
    );

    let reg = LanguageRegistry::new_tier1().unwrap();
    let interner = Interner::new();
    let ingest: IngestResult = ingest_crawled_files(&crawled, &reg, &interner);
    assert_eq!(ingest.file_count(), 5);

    let sym_names: Vec<&str> = ingest
        .symbols
        .iter()
        .map(|s| interner.resolve(&s.name))
        .collect();
    for expected in [
        "AuthManager",
        "verify",
        "Database",
        "connect",
        "Router",
        "route",
        "Config",
        "Server",
        "start",
        "Service",
        "Run",
    ] {
        assert!(sym_names.contains(&expected), "missing symbol: {expected}");
    }

    let ref_callees: Vec<&str> = ingest
        .references
        .iter()
        .map(|r| interner.resolve(&r.callee_name))
        .collect();
    for expected in [
        "check_token",
        "log_info",
        "dispatch",
        "handleRequest",
        "helper",
    ] {
        assert!(
            ref_callees.contains(&expected),
            "missing reference: {expected}"
        );
    }

    let connect_sym = ingest
        .symbols
        .iter()
        .find(|s| interner.resolve(&s.name) == "connect")
        .unwrap();
    let log_ref = ingest
        .references
        .iter()
        .find(|r| interner.resolve(&r.callee_name) == "log_info")
        .unwrap();
    assert_eq!(log_ref.from_symbol, connect_sym.id);
    assert_eq!(log_ref.role, RefRole::Call);
    assert_eq!(connect_sym.kind, SymKind::Method);
}
