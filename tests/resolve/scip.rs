use protobuf::Message;
use ripwire_rs::cli::{NavMode, execute_nav_symbol_query};
use ripwire_rs::resolve::scip_sites;
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, PageRankConfig, ingest_in_memory_files};
use scip::types::{Document, Index, Occurrence, SymbolRole};

const SOURCES: &[(&str, Lang, &[u8])] = &[
    ("a/x.py", Lang::Python, b"def run():\n    return 1\n"),
    ("b/y.py", Lang::Python, b"def run():\n    return 2\n"),
    (
        "c/use.py",
        Lang::Python,
        b"def go(o):\n    return o.run()\n\ndef ext():\n    return run()\n",
    ),
];

fn occurrence(symbol: &str, line: i32, definition: bool) -> Occurrence {
    Occurrence {
        symbol: symbol.to_string(),
        range: vec![line, 4, 7],
        symbol_roles: if definition {
            SymbolRole::Definition as i32
        } else {
            0
        },
        ..Occurrence::default()
    }
}

fn document(path: &str, occurrences: Vec<Occurrence>) -> Document {
    Document {
        relative_path: path.to_string(),
        occurrences,
        ..Document::default()
    }
}

const X_RUN: &str = "scip-python python app 1.0 a/x/run().";
const Y_RUN: &str = "scip-python python app 1.0 b/y/run().";
const LIB_RUN: &str = "scip-python python lib 2.0 lib/run().";

fn index_bytes() -> Vec<u8> {
    Index {
        documents: vec![
            document("a/x.py", vec![occurrence(X_RUN, 0, true)]),
            document("b/y.py", vec![occurrence(Y_RUN, 0, true)]),
            document(
                "c/use.py",
                vec![occurrence(Y_RUN, 1, false), occurrence(LIB_RUN, 4, false)],
            ),
        ],
        ..Index::default()
    }
    .write_to_bytes()
    .unwrap()
}

fn callees(sym: &str, scip: Option<&[u8]>) -> String {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let mut ingest = ingest_in_memory_files(SOURCES, &registry, &interner);
    if let Some(bytes) = scip {
        ingest.scip_sites = scip_sites(bytes, &ingest, &interner).unwrap();
    }
    let graph = Graph::build(&ingest, &interner);
    let ranks = graph.page_rank(&PageRankConfig::default()).ranks;
    let mut out = Vec::new();
    execute_nav_symbol_query(
        sym,
        NavMode::Callees,
        &ingest,
        &graph,
        &ranks,
        &interner,
        &mut out,
    )
    .unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn scip_index_pins_call_sites_the_name_ladder_declines() {
    let without = callees("go", None);
    assert!(without.contains("declined_calls=\"1\""), "{without}");
    let bytes = index_bytes();
    let with = callees("go", Some(&bytes));
    assert!(
        with.contains("<s t=\"fn\" n=\"run\" p=\"b/y.py:1\" prov=\"scip\"/>"),
        "{with}"
    );
    assert!(!with.contains("a/x.py"), "{with}");
    assert!(!with.contains("declined_calls=\""), "{with}");
}

#[test]
fn scip_symbol_defined_outside_the_index_is_external() {
    let bytes = index_bytes();
    let with = callees("ext", Some(&bytes));
    assert!(with.contains("count=\"0\""), "{with}");
    assert!(!with.contains("declined_calls=\""), "{with}");
}

#[test]
fn non_scip_bytes_are_rejected() {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(SOURCES, &registry, &interner);
    assert!(scip_sites(b"\xff\xff\xff\xff", &ingest, &interner).is_err());
}
