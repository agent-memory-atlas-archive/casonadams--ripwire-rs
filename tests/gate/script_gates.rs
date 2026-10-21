use ripwire_rs::gate::AffectedTest;
use ripwire_rs::gate::script_gates::classify_script_gates;

#[test]
fn test_registered_gates_map_by_exact_indexed_paths() {
    let scripts = [
        (
            "test/regression.sh",
            "bash test/lit.sh\nsh \"$(dirname \"$0\")/deps.sh\"\n./test/dyn.sh\nbash test/base.sh\n",
        ),
        ("test/lit.sh", "cat src/a.c\n"),
        ("test/deps.sh", "RIPWIRE_TEST_DEPS=\"src/a.c src/b.c\"\n"),
        ("test/dyn.sh", "echo \"$X\"\n"),
        ("test/base.sh", "cat a.c src/a.cc lib/src/b.cpp\n"),
        ("test/unregistered.sh", "cat src/a.c\n"),
        ("test/split.sh", "cat src/a.c\n"),
    ];
    let indexed = ["src/a.c", "src/b.c", "src/a.cc"];
    let gates = classify_script_gates(&scripts, &indexed);
    assert_eq!(gates.unmodelled, 7);
    let registered: Vec<(&str, Vec<&str>)> = gates
        .registered
        .iter()
        .map(|(g, d)| (g.as_str(), d.iter().map(String::as_str).collect()))
        .collect();
    assert_eq!(
        registered,
        [
            ("test/lit.sh", vec!["src/a.c"]),
            ("test/deps.sh", vec!["src/a.c", "src/b.c"]),
            ("test/dyn.sh", vec![]),
            ("test/base.sh", vec!["src/a.cc"]),
        ]
    );
    assert_eq!(gates.mapped(), 3);
    let listed = [AffectedTest::new("test/deps.sh".into(), 1, false, false)];
    let rows: Vec<String> = gates
        .covering_rows(&["src/a.c".into()], &listed)
        .into_iter()
        .filter(|t| t.script_literal)
        .map(|t| t.path)
        .collect();
    assert_eq!(rows, ["test/lit.sh"]);
    assert!(
        classify_script_gates(&scripts[1..], &indexed)
            .registered
            .is_empty()
    );
}
