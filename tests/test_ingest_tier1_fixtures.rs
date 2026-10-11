use rwire::{
    ingest_in_memory_files, IngestResult, Interner, Lang, LanguageRegistry, RefRole, SymKind,
};

#[test]
fn test_ingest_tier1_fixtures() {
    let fixtures: &[(&str, Lang, &[u8])] = &[
        (
            "auth.py",
            Lang::Python,
            b"class AuthManager:\n    def verify(self):\n        check_token()\ndef check_token():\n    pass\n",
        ),
        (
            "db.rs",
            Lang::Rust,
            b"pub struct Database; impl Database { pub fn connect(&self) { log_info(); } } fn log_info() {}",
        ),
        (
            "router.js",
            Lang::JavaScript,
            b"class Router { route() { dispatch(); } } function dispatch() {}",
        ),
        (
            "server.ts",
            Lang::TypeScript,
            b"export interface Config {} export class Server { start() { handleRequest(); } } function handleRequest() {}",
        ),
        (
            "service.go",
            Lang::Go,
            b"package main\ntype Service struct{}\nfunc (s *Service) Run() { helper() }\nfunc helper() {}\n",
        ),
    ];

    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest: IngestResult = ingest_in_memory_files(fixtures, &reg, &interner);
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
