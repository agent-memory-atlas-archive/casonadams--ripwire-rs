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

    // 1. Rust fixture
    let rs_code = r#"
        pub struct Database {
            url: String,
        }

        impl Database {
            pub fn connect(&self) {
                log_info();
            }
        }

        fn log_info() {}
    "#;
    fs::write(root.join("db.rs"), rs_code).unwrap();

    // 2. TypeScript fixture
    let ts_code = r#"
        export interface Config {
            port: number;
        }

        export class Server {
            start() {
                handleRequest();
            }
        }

        function handleRequest() {}
    "#;
    fs::write(root.join("server.ts"), ts_code).unwrap();

    // 3. JavaScript fixture
    let js_code = r#"
        class Router {
            route() {
                dispatch();
            }
        }

        function dispatch() {}
    "#;
    fs::write(root.join("router.js"), js_code).unwrap();

    // 4. Python fixture
    let py_code = r#"
class AuthManager:
    def verify(self):
        check_token()

def check_token():
    pass
"#;
    fs::write(root.join("auth.py"), py_code).unwrap();

    // 5. Go fixture
    let go_code = r#"
        package main

        type Service struct {
            Name string
        }

        func (s *Service) Run() {
            helper()
        }

        func helper() {}
    "#;
    fs::write(root.join("service.go"), go_code).unwrap();

    // Crawl the fixture directory
    let crawled_files = crawl_directory(root, &CrawlerOptions::default());
    assert_eq!(crawled_files.len(), 5);

    // Verify determinism: files must be in lexicographical order by relative path
    let rel_paths: Vec<&str> = crawled_files.iter().map(|f| f.rel_path.as_str()).collect();
    assert_eq!(
        rel_paths,
        vec!["auth.py", "db.rs", "router.js", "server.ts", "service.go"]
    );

    // Ingest all files in parallel
    let registry = LanguageRegistry::new_tier1().unwrap();
    let interner = Interner::new();
    let ingest: IngestResult = ingest_crawled_files(&crawled_files, &registry, &interner);

    assert_eq!(ingest.file_count(), 5);

    // Check extracted symbols
    let sym_names: Vec<&str> = ingest
        .symbols
        .iter()
        .map(|s| interner.resolve(&s.name))
        .collect();

    // Python symbols
    assert!(sym_names.contains(&"AuthManager"));
    assert!(sym_names.contains(&"verify"));
    assert!(sym_names.contains(&"check_token"));

    // Rust symbols
    assert!(sym_names.contains(&"Database"));
    assert!(sym_names.contains(&"connect"));
    assert!(sym_names.contains(&"log_info"));

    // JavaScript symbols
    assert!(sym_names.contains(&"Router"));
    assert!(sym_names.contains(&"route"));
    assert!(sym_names.contains(&"dispatch"));

    // TypeScript symbols
    assert!(sym_names.contains(&"Config"));
    assert!(sym_names.contains(&"Server"));
    assert!(sym_names.contains(&"start"));
    assert!(sym_names.contains(&"handleRequest"));

    // Go symbols
    assert!(sym_names.contains(&"Service"));
    assert!(sym_names.contains(&"Run"));
    assert!(sym_names.contains(&"helper"));

    // Check references and attribution
    let ref_callees: Vec<&str> = ingest
        .references
        .iter()
        .map(|r| interner.resolve(&r.callee_name))
        .collect();

    assert!(ref_callees.contains(&"check_token"));
    assert!(ref_callees.contains(&"log_info"));
    assert!(ref_callees.contains(&"dispatch"));
    assert!(ref_callees.contains(&"handleRequest"));
    assert!(ref_callees.contains(&"helper"));

    // Verify that enclosing symbols were assigned (from_symbol != NO_NODE for methods)
    let connect_sym = ingest
        .symbols
        .iter()
        .find(|s| interner.resolve(&s.name) == "connect")
        .unwrap();

    let log_info_ref = ingest
        .references
        .iter()
        .find(|r| interner.resolve(&r.callee_name) == "log_info")
        .unwrap();

    assert_eq!(log_info_ref.from_symbol, connect_sym.id);
    assert_eq!(log_info_ref.role, RefRole::Call);
    assert_eq!(connect_sym.kind, SymKind::Method);
}
