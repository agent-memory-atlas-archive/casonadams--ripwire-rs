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
        "a/zammad.dart",
        Lang::Dart,
        b"class Zammad { void createUser(String n) {} }\n",
    ),
    (
        "b/keycloak.dart",
        Lang::Dart,
        b"class Keycloak { void createUser(String n) {} }\n",
    ),
    (
        "c/provision.dart",
        Lang::Dart,
        b"void provisionMember(dynamic c, String n) {\n  c.createUser(n);\n}\n",
    ),
];

#[test]
fn callers_disclose_declined_call_sites() {
    let xml = nav_xml(AMBIGUOUS, "createUser", NavMode::Callers).unwrap();
    assert!(xml.contains("declined_calls=\"1\""), "{xml}");
    assert!(
        xml.contains(
            "<dc in=\"provisionMember\" p=\"c/provision.dart:2\" cands=\"2\" via=\"declined\" of_def=\"1~d 2~d\"/>"
        ),
        "{xml}"
    );
    assert!(
        xml.contains("<def ix=\"1\" t=\"method\" sc=\"Zammad\" p=\"a/zammad.dart:1\"/>"),
        "{xml}"
    );
    assert!(xml.contains("<def ix=\"2\""), "{xml}");
}

#[test]
fn callees_disclose_declined_call_sites() {
    let xml = nav_xml(AMBIGUOUS, "provisionMember", NavMode::Callees).unwrap();
    assert!(xml.contains("declined_calls=\"1\""), "{xml}");
    assert!(
        xml.contains("<dc n=\"createUser\" l=\"2\" cands=\"2\" via=\"declined\"/>"),
        "{xml}"
    );
}

#[test]
fn uses_of_undefined_symbol_is_external_not_an_error() {
    let fixtures: &[(&str, Lang, &[u8])] =
        &[("m.dart", Lang::Dart, b"void m() { final x = Member(1); }\n")];
    let xml = nav_xml(fixtures, "Member", NavMode::Uses).unwrap();
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
            "a/zammad.dart",
            Lang::Dart,
            b"class Zammad { void createUser(String n) {} }\n",
        ),
        (
            "b/keycloak.dart",
            Lang::Dart,
            b"class Keycloak { void createUser(String n) {} }\n",
        ),
        (
            "b/use.dart",
            Lang::Dart,
            b"void viaKeycloak(Keycloak k) {\n  Keycloak.createUser('x');\n}\n",
        ),
    ];
    let xml = nav_xml(fixtures, "createUser", NavMode::Callers).unwrap();
    assert!(
        xml.contains("n=\"viaKeycloak\"") && xml.contains("of_def=\"2\""),
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
            b"void load() {\n  final q = \"SELECT ${common.Member.cols().join(', ')}\";\n}\n",
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
