use ripwire_rs::metrics::nonlocal::{NonlocalFunction, find_nonlocal_state};
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, ingest_in_memory_files};

const MODULE: &str = "src/module.py";

fn state_report(files: &[(&str, &str)]) -> Vec<NonlocalFunction> {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let sources: Vec<(&str, Lang, &[u8])> = files
        .iter()
        .map(|(path, src)| (*path, Lang::Python, src.as_bytes()))
        .collect();
    let ingest = ingest_in_memory_files(&sources, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    find_nonlocal_state(&ingest, &graph, &interner)
}

fn function<'a>(report: &'a [NonlocalFunction], name: &str) -> Option<&'a NonlocalFunction> {
    report.iter().find(|f| f.sym_name == name)
}

const CONSTANTS: &str = "from collections import namedtuple
LIMIT = 30
LABEL = \"x\"
ON = True
NOTHING = None
NEGATIVE = -1
KINDS = (\"a\", \"b\")
Outcome = namedtuple(\"Outcome\", \"reason rw\")
";

const READS_CONSTANTS: &str =
    "from module import LIMIT, LABEL, ON, NOTHING, NEGATIVE, KINDS, Outcome
def run():
    return LIMIT + NEGATIVE, LABEL, ON, NOTHING, KINDS, Outcome(None, None)
";

#[test]
fn reads_of_python_constants_and_type_factories_are_not_state() {
    let report = state_report(&[(MODULE, CONSTANTS), ("src/use.py", READS_CONSTANTS)]);
    assert!(
        report.is_empty(),
        "immutable python reads reported as state: {:?}",
        report.iter().map(|f| &f.sym_name).collect::<Vec<_>>()
    );
}

const SAME_MODULE_MUTABLE: &str = "CACHE = []
COUNT = 0
def remember(item):
    CACHE.append(item)
def bump():
    global COUNT
    COUNT = COUNT + 1
def read_count():
    return COUNT
";

#[test]
fn python_globals_rebound_by_their_own_module_are_state() {
    let report = state_report(&[(MODULE, SAME_MODULE_MUTABLE)]);
    let remember = function(&report, "remember").expect("remember mutates CACHE");
    assert!(remember.cells.iter().any(|c| c.cell_name == "CACHE"));
    let read = function(&report, "read_count").expect("COUNT is rebound by a global statement");
    assert!(
        read.cells
            .iter()
            .any(|c| c.cell_name == "COUNT" && !c.is_write)
    );
}
