use ripwire_rs::gate::seams::find_integration_seams;
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, ingest_in_memory_files};

#[test]
fn test_find_integration_seams_across_directories() {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[
            (
                "src/gate/seams.rs",
                Lang::Rust,
                b"pub fn caller_fn() { callee_fn(); }\n",
            ),
            ("lib.rs", Lang::Rust, b"pub fn callee_fn() {}\n"),
        ],
        &reg,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    let rep = find_integration_seams(&ingest, &graph, &interner);
    assert_eq!(rep.groups.len(), 1);
    assert_eq!(rep.groups[0].from_dir, "src/gate");
    assert_eq!(rep.groups[0].to_dir, ".");
}
