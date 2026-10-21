use ripwire_rs::cli::{NavMode, execute_nav_symbol_query};
use ripwire_rs::{Graph, Lang, LanguageRegistry, ingest_in_memory_files};
use ripwire_rs::{Interner, PageRankConfig};

fn nav_xml(fixtures: &[(&str, Lang, &[u8])], sym: &str, mode: NavMode) -> std::io::Result<String> {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(fixtures, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let ranks = graph.page_rank(&PageRankConfig::default()).ranks;
    let mut out = Vec::new();
    execute_nav_symbol_query(sym, mode, &ingest, &graph, &ranks, &interner, &mut out)?;
    Ok(String::from_utf8(out).unwrap())
}

const AMBIGUOUS: &[(&str, Lang, &[u8])] = &[
    (
        "a/alpha.dart",
        Lang::Dart,
        b"class Alpha { void register(String n) {} }\n",
    ),
    (
        "b/beta.dart",
        Lang::Dart,
        b"class Beta { void register(String n) {} }\n",
    ),
    (
        "c/setup.dart",
        Lang::Dart,
        b"void setup(dynamic c, String n) {\n  c.register(n);\n}\n",
    ),
];

#[test]
fn callers_disclose_declined_call_sites() {
    let xml = nav_xml(AMBIGUOUS, "register", NavMode::Callers).unwrap();
    assert!(xml.contains("declined_calls=\"1\""), "{xml}");
    assert!(
        xml.contains(
            "<dc in=\"setup\" p=\"c/setup.dart:2\" cands=\"2\" via=\"declined\" of_def=\"1~d 2~d\"/>"
        ),
        "{xml}"
    );
    assert!(
        xml.contains("<def ix=\"1\" t=\"method\" sc=\"Alpha\" p=\"a/alpha.dart:1\"/>"),
        "{xml}"
    );
    assert!(xml.contains("<def ix=\"2\""), "{xml}");
}

#[test]
fn callees_disclose_declined_call_sites() {
    let xml = nav_xml(AMBIGUOUS, "setup", NavMode::Callees).unwrap();
    assert!(xml.contains("declined_calls=\"1\""), "{xml}");
    assert!(
        xml.contains("<dc n=\"register\" l=\"2\" cands=\"2\" via=\"declined\"/>"),
        "{xml}"
    );
}

#[test]
fn uses_of_undefined_symbol_is_external_not_an_error() {
    let fixtures: &[(&str, Lang, &[u8])] =
        &[("m.dart", Lang::Dart, b"void m() { final x = Widget(1); }\n")];
    let xml = nav_xml(fixtures, "Widget", NavMode::Uses).unwrap();
    assert!(
        xml.contains("defs=\"0\" external=\"1\" count=\"1\""),
        "{xml}"
    );
    assert!(nav_xml(fixtures, "Unreferenced", NavMode::Uses).is_err());
}

#[test]
fn callers_tag_each_row_with_the_definition_it_calls() {
    let fixtures: &[(&str, Lang, &[u8])] = &[
        (
            "a/alpha.dart",
            Lang::Dart,
            b"class Alpha { void register(String n) {} }\n",
        ),
        (
            "b/beta.dart",
            Lang::Dart,
            b"class Beta { void register(String n) {} }\n",
        ),
        (
            "b/use.dart",
            Lang::Dart,
            b"void viaBeta(Beta k) {\n  Beta.register('x');\n}\n",
        ),
    ];
    let xml = nav_xml(fixtures, "register", NavMode::Callers).unwrap();
    assert!(
        xml.contains("n=\"viaBeta\"") && xml.contains("of_def=\"2\""),
        "{xml}"
    );
}

#[test]
fn type_qualified_call_to_unindexed_type_is_external_not_declined() {
    let fixtures: &[(&str, Lang, &[u8])] = &[
        (
            "a/x.dart",
            Lang::Dart,
            b"class X { static List<String> cols() => []; }\n",
        ),
        (
            "b/y.dart",
            Lang::Dart,
            b"class Y { static List<String> cols() => []; }\n",
        ),
        (
            "c/load.dart",
            Lang::Dart,
            b"void load() {\n  final q = \"SELECT ${pkg.Widget.cols().join(', ')}\";\n}\n",
        ),
    ];
    let xml = nav_xml(fixtures, "load", NavMode::Callees).unwrap();
    assert!(!xml.contains("declined_calls=\""), "{xml}");
    assert!(!xml.contains("n=\"cols\""), "{xml}");
}

#[test]
fn ambiguous_call_arms_are_labelled_split() {
    let fixtures: &[(&str, Lang, &[u8])] = &[
        ("a/x.dart", Lang::Dart, b"class X { void run() {} }\n"),
        ("a/y.dart", Lang::Dart, b"class Y { void run() {} }\n"),
        (
            "a/use.dart",
            Lang::Dart,
            b"void go(dynamic c) {\n  c.run();\n}\n",
        ),
    ];
    let callees = nav_xml(fixtures, "go", NavMode::Callees).unwrap();
    assert_eq!(callees.matches("prov=\"split\"").count(), 2, "{callees}");
    let callers = nav_xml(fixtures, "run", NavMode::Callers).unwrap();
    assert!(callers.contains("of_def=\"1~s 2~s\""), "{callers}");
}

#[test]
fn callee_rows_label_how_each_edge_bound() {
    let fixtures: &[(&str, Lang, &[u8])] = &[
        (
            "pkg/util.py",
            Lang::Python,
            b"def helper():\n    return 1\n",
        ),
        (
            "other/util.py",
            Lang::Python,
            b"def helper():\n    return 2\n",
        ),
        (
            "app.py",
            Lang::Python,
            b"import pkg.util as u\n\ndef main():\n    return u.helper() + solo()\n",
        ),
        ("lib/solo.py", Lang::Python, b"def solo():\n    return 3\n"),
        (
            "a/x.dart",
            Lang::Dart,
            b"class X { static int cols() => 1; }\n",
        ),
        (
            "a/y.dart",
            Lang::Dart,
            b"class Y { static int cols() => 2; }\n",
        ),
        (
            "b/load.dart",
            Lang::Dart,
            b"int load() {\n  return X.cols();\n}\n",
        ),
    ];
    let py = nav_xml(fixtures, "main", NavMode::Callees).unwrap();
    assert!(
        py.contains("n=\"helper\" p=\"pkg/util.py:1\" prov=\"import\"/>"),
        "{py}"
    );
    assert!(
        py.contains("n=\"solo\" p=\"lib/solo.py:1\" via=\"name\"/>"),
        "{py}"
    );
    let dart = nav_xml(fixtures, "load", NavMode::Callees).unwrap();
    assert!(
        dart.contains("n=\"cols\" p=\"a/x.dart:1\" prov=\"final-segment\"/>"),
        "{dart}"
    );
}

#[test]
fn caller_rows_mark_name_only_bindings_with_tilde_n() {
    let fixtures: &[(&str, Lang, &[u8])] = &[
        (
            "a/one.py",
            Lang::Python,
            b"def run():\n    return 1\n\ndef same_file():\n    return run()\n",
        ),
        (
            "a/three.py",
            Lang::Python,
            b"def same_dir():\n    return run()\n",
        ),
        ("b/two.py", Lang::Python, b"def run():\n    return 2\n"),
    ];
    let xml = nav_xml(fixtures, "run", NavMode::Callers).unwrap();
    assert!(
        xml.contains("<s t=\"fn\" n=\"same_file\" p=\"a/one.py:4\" of_def=\"1\""),
        "{xml}"
    );
    assert!(
        xml.contains("<s t=\"fn\" n=\"same_dir\" p=\"a/three.py:1\" via=\"name\" of_def=\"1~n\""),
        "{xml}"
    );
}

#[test]
fn self_receiver_calls_bind_lexically_without_a_label() {
    let fixtures: &[(&str, Lang, &[u8])] = &[
        (
            "b/k.py",
            Lang::Python,
            b"class K:\n    def go(self):\n        return 1\n\n    def me(self):\n        return self.go()\n",
        ),
        ("b/l.py", Lang::Python, b"class L:\n    def go(self):\n        return 2\n"),
    ];
    let xml = nav_xml(fixtures, "me", NavMode::Callees).unwrap();
    assert!(
        xml.contains("<s t=\"fn\" n=\"go\" p=\"b/k.py:2\"/>"),
        "{xml}"
    );
}

const PYBIND: &[u8] = b"#include <pybind11/pybind11.h>
namespace py = pybind11;
int add_impl(int a, int b) { return a + b; }
int add(int a, int b) { return a - b; }
int plain(int a) { return a; }
struct Pet { void bark() {} };
PYBIND11_MODULE(native, m) {
    m.def(\"add\", &add_impl);
    m.def(\"plain_fn\", plain);
    py::class_<Pet>(m, \"Pet\").def(py::init<>()).def(\"speak\", &Pet::bark);
}
";

#[test]
fn python_calls_bind_to_pybind11_exports() {
    let fixtures: &[(&str, Lang, &[u8])] = &[
        ("bind.cpp", Lang::Cpp, PYBIND),
        (
            "use.py",
            Lang::Python,
            b"import native\n\ndef free_call():\n    return native.add(1, 2)\n\ndef method_call(p):\n    return p.speak()\n\ndef unexported():\n    return native.plain_fn(1)\n",
        ),
    ];
    let free = nav_xml(fixtures, "free_call", NavMode::Callees).unwrap();
    assert!(
        free.contains("<s t=\"fn\" n=\"add_impl\" p=\"bind.cpp:3\" prov=\"binding\"/>"),
        "{free}"
    );
    assert!(!free.contains("n=\"add\" "), "{free}");
    let method = nav_xml(fixtures, "method_call", NavMode::Callees).unwrap();
    assert!(
        method.contains("n=\"bark\"") && method.contains("prov=\"binding\""),
        "{method}"
    );
    let unexported = nav_xml(fixtures, "unexported", NavMode::Callees).unwrap();
    assert!(unexported.contains("count=\"0\""), "{unexported}");
}

#[test]
fn same_language_definition_wins_over_a_binding_and_no_module_means_no_export() {
    let fixtures: &[(&str, Lang, &[u8])] = &[
        ("bind.cpp", Lang::Cpp, PYBIND),
        (
            "use.py",
            Lang::Python,
            b"def add(a, b):\n    return a\n\ndef local_call():\n    return add(1, 2)\n",
        ),
        (
            "plain.cpp",
            Lang::Cpp,
            b"int helper_impl() { return 1; }\nvoid reg(Registry& r) { r.def(\"helper\", &helper_impl); }\n",
        ),
        ("use2.py", Lang::Python, b"def other():\n    return lib.helper()\n"),
    ];
    let local = nav_xml(fixtures, "local_call", NavMode::Callees).unwrap();
    assert!(local.contains("n=\"add\" p=\"use.py:1\""), "{local}");
    assert!(!local.contains("prov=\"binding\""), "{local}");
    let other = nav_xml(fixtures, "other", NavMode::Callees).unwrap();
    assert!(other.contains("count=\"0\""), "{other}");
}
