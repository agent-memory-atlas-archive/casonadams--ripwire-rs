use ripwire_rs::{Graph, IngestResult, Interner, Lang, LanguageRegistry, ingest_in_memory_files};

/// The files of every symbol `caller` calls, by the callee's file path.
fn callee_files(files: &[(&str, Lang, &[u8])], caller: &str) -> Vec<String> {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest: IngestResult = ingest_in_memory_files(files, &reg, &interner);
    let graph = Graph::build(&ingest, &interner);
    let caller_id = ingest
        .symbols
        .iter()
        .find(|s| interner.resolve(&s.name) == caller)
        .map(|s| s.id)
        .unwrap();
    let mut out: Vec<String> = graph
        .callees(caller_id)
        .map(|(callee, _)| {
            let sym = &ingest.symbols[callee as usize];
            ingest.sym_file_path(sym, &interner).to_string()
        })
        .collect();
    out.sort();
    out
}

const MODULES: [(&str, Lang, &[u8]); 2] = [
    (
        "src/commands/ast_match.rs",
        Lang::Rust,
        b"pub fn execute() {}\n",
    ),
    ("src/other.rs", Lang::Rust, b"pub fn execute() {}\n"),
];

#[test]
fn test_grouped_use_binds_module_qualifier() {
    let caller = (
        "tests/cli/retrieval.rs",
        Lang::Rust,
        &b"use ripwire_rs::commands::{ast_match, at};\nfn probe() { ast_match::execute(); }\n"[..],
    );
    let files = [caller, MODULES[0], MODULES[1]];
    assert_eq!(
        callee_files(&files, "probe"),
        ["src/commands/ast_match.rs"],
        "the grouped module binding resolves the qualifier to its own file only"
    );
}

#[test]
fn test_renamed_group_member_binds_its_alias() {
    let caller = (
        "tests/cli/retrieval.rs",
        Lang::Rust,
        &b"use ripwire_rs::commands::{ast_match as am};\nfn probe() { am::execute(); }\n"[..],
    );
    let files = [caller, MODULES[0], MODULES[1]];
    assert_eq!(callee_files(&files, "probe"), ["src/commands/ast_match.rs"]);
}

#[test]
fn test_unbound_qualifier_does_not_link_a_same_name_function() {
    let caller = (
        "tests/cli/retrieval.rs",
        Lang::Rust,
        &b"use ripwire_rs::commands::at;\nfn probe() { ast_match::execute(); }\n"[..],
    );
    let files = [caller, MODULES[0], MODULES[1]];
    assert!(
        callee_files(&files, "probe").is_empty(),
        "`ast_match` is not bound by the import, so `execute` must not link to any module"
    );
}
