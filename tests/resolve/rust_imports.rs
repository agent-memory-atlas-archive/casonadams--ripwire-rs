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

#[test]
fn test_module_path_call_binds_the_module_function_not_a_trait_method_of_that_name() {
    let files: [(&str, Lang, &[u8]); 3] = [
        (
            "src/commands/mod.rs",
            Lang::Rust,
            b"pub trait Command { fn run(&self, root: &str) -> Option<u8>; }\npub fn run(root: &str) -> Option<u8> { None }\n",
        ),
        (
            "src/commands/alpha.rs",
            Lang::Rust,
            b"struct Alpha;\nimpl crate::commands::Command for Alpha { fn run(&self, root: &str) -> Option<u8> { None } }\n",
        ),
        (
            "src/cli/dispatch.rs",
            Lang::Rust,
            b"pub fn probe(root: &str) { crate::commands::run(root); }\n",
        ),
    ];
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(&files, &reg, &interner);
    let graph = Graph::build(&ingest, &interner);
    let probe = ingest
        .symbols
        .iter()
        .find(|s| interner.resolve(&s.name) == "probe")
        .unwrap()
        .id;
    let callees: Vec<(u32, bool)> = graph
        .callees(probe)
        .map(|(id, _)| {
            let sym = &ingest.symbols[id as usize];
            (sym.line, sym.scope.is_some())
        })
        .collect();
    assert_eq!(
        callees,
        [(2, false)],
        "only the free `run` in src/commands/mod.rs"
    );
}

#[test]
fn test_a_method_call_never_binds_a_module_declaration_of_that_name() {
    let files: [(&str, Lang, &[u8]); 2] = [
        (
            "src/query/mod.rs",
            Lang::Rust,
            b"pub mod map;\npub mod json;\n",
        ),
        (
            "src/app.rs",
            Lang::Rust,
            b"fn probe(v: Vec<u8>) { let _ = v.iter().map(|x| x + 1).count(); }\n",
        ),
    ];
    assert!(
        callee_files(&files, "probe").is_empty(),
        "`.map()` on an iterator is not the `mod map;` declaration"
    );
}
