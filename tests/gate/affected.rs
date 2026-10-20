use ripwire_rs::{
    FileInfo, Graph, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol,
    ToXmlString, find_affected_tests, is_test_path,
};

#[test]
fn test_is_test_path_detection() {
    assert!(is_test_path("tests/foo.rs"));
    assert!(is_test_path("src/lib_test.go"));
    assert!(is_test_path("ui/component.test.ts"));
    assert!(is_test_path("core/test_engine.rs"));
    assert!(!is_test_path("src/lib.rs"));
    assert!(!is_test_path("src/engine.rs"));
}

#[test]
fn test_affected_transitive_reachability() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();

    let p_core = interner.intern("src/core.rs");
    let p_service = interner.intern("src/service.rs");
    let p_test = interner.intern("tests/test_service.rs");

    ingest.files.push(FileInfo {
        id: 0,
        path: p_core,
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    });
    ingest.files.push(FileInfo {
        id: 1,
        path: p_service,
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    });
    ingest.files.push(FileInfo {
        id: 2,
        path: p_test,
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    });

    let s0 = Symbol::new(
        0,
        interner.intern("core_fn"),
        interner.intern("core_fn"),
        SymKind::Function,
        Lang::Rust,
        0,
        5,
    );
    let s1 = Symbol::new(
        1,
        interner.intern("service_fn"),
        interner.intern("service_fn"),
        SymKind::Function,
        Lang::Rust,
        1,
        10,
    );
    let s2 = Symbol::new(
        2,
        interner.intern("test_fn"),
        interner.intern("test_fn"),
        SymKind::Function,
        Lang::Rust,
        2,
        15,
    );

    ingest.symbols.push(s0);
    ingest.symbols.push(s1);
    ingest.symbols.push(s2);

    ingest.references.push(Reference::new(
        1,
        interner.intern("core_fn"),
        1,
        12,
        Lang::Rust,
        RefRole::Call,
    ));
    ingest.references.push(Reference::new(
        2,
        interner.intern("service_fn"),
        2,
        18,
        Lang::Rust,
        RefRole::Call,
    ));

    let graph = Graph::build(&ingest, &interner);

    let report = find_affected_tests(&["src/core.rs"], &ingest, &graph, &interner);
    assert_eq!(report.seed_count, 1);
    assert_eq!(report.total_tests, 1);
    assert_eq!(report.tests[0].path, "tests/test_service.rs");
    assert_eq!(report.tests[0].hops, 2);

    let xml = report.to_xml_string();
    assert!(xml.contains("<affected schema=\"ripwire-rs.affected/v1\""));
    assert!(xml.contains("<test p=\"tests/test_service.rs\" hops=\"2\" run_unknown=\"1\"/>"));
}

#[test]
fn test_partner_subject_stem_naming_cases() {
    use ripwire_rs::gate::affected::partner_subject_stem;
    let cases: [(&str, Option<&str>); 22] = [
        (
            "scripts/test_lint_bronze_contract.py",
            Some("lint_bronze_contract"),
        ),
        ("src/lint_test.py", Some("lint")),
        ("pkg/ring.test.ts", Some("ring")),
        ("pkg/markdown.test.tsx", Some("markdown")),
        ("pkg/dirs.spec.js", Some("dirs")),
        ("src/server_test.go", Some("server")),
        ("src/widget_test.rs", Some("widget")),
        ("spec/widget_spec.rb", Some("widget")),
        ("tests/test_widget.ts", Some("widget")),
        ("src/widget_test.cpp", Some("widget")),
        ("tests/WidgetTest.java", Some("Widget")),
        ("src/test/java/WidgetTest.java", Some("Widget")),
        ("tests/WidgetTest.kt", Some("Widget")),
        ("tests/WidgetTest.php", Some("Widget")),
        ("src/WidgetTest.java", None),
        ("tests/WidgetTest.py", None),
        ("test/WidgetTest.scala", None),
        ("tests/widget.rs", None),
        ("src/widget_unittest.cc", None),
        ("tests/Widget_Test.php", None),
        ("scripts/test_.py", None),
        ("Makefile", None),
    ];
    for (path, want) in cases {
        let lang = Lang::from_path(std::path::Path::new(path));
        assert_eq!(partner_subject_stem(path, lang), want, "{path}");
    }
}

fn partner_fixture(paths: &[&str]) -> (IngestResult, Interner) {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    for (id, p) in paths.iter().enumerate() {
        ingest.files.push(FileInfo {
            id: u32::try_from(id).unwrap_or(u32::MAX),
            path: interner.intern(p),
            lang: Lang::from_path(std::path::Path::new(p)),
            line_count: 10,
            byte_size: 100,
        });
    }
    (ingest, interner)
}

fn affected_paths(paths: &[&str], changed: &str) -> Vec<String> {
    let (ingest, interner) = partner_fixture(paths);
    let graph = Graph::build(&ingest, &interner);
    let report = find_affected_tests(&[changed], &ingest, &graph, &interner);
    report.tests.iter().map(|t| t.path.clone()).collect()
}

#[test]
fn test_affected_partner_row_for_unique_convention_match() {
    let (ingest, interner) = partner_fixture(&[
        "scripts/lint_bronze_contract.py",
        "scripts/test_lint_bronze_contract.py",
        "scripts/test_lint_bronze_contract_extra.py",
        "tests/lint_bronze_contract.py",
        "jobs/a/lint_bronze_contract.py",
    ]);
    let graph = Graph::build(&ingest, &interner);
    let report = find_affected_tests(
        &["scripts/lint_bronze_contract.py"],
        &ingest,
        &graph,
        &interner,
    );
    let paths: Vec<&str> = report.tests.iter().map(|t| t.path.as_str()).collect();
    assert_eq!(paths, ["scripts/test_lint_bronze_contract.py"]);
    assert!(report.tests[0].is_partner && report.tests[0].hops == 0);
    assert!(report.to_xml_string().contains(
        "<test p=\"scripts/test_lint_bronze_contract.py\" partner=\"1\" run_unknown=\"1\"/>"
    ));
}

#[test]
fn test_affected_partner_cross_language_and_cross_package() {
    assert_eq!(
        affected_paths(
            &["pkg1/src/env.ts", "pkg2/test/env.test.ts"],
            "pkg1/src/env.ts"
        ),
        ["pkg2/test/env.test.ts"]
    );
    assert_eq!(
        affected_paths(&["src/widget.py", "tests/test_widget.ts"], "src/widget.py"),
        ["tests/test_widget.ts"]
    );
}

#[test]
fn test_affected_partner_declined_when_ambiguous_or_unnamed() {
    let cases: [(&[&str], &str); 4] = [
        (
            &["a/widget.py", "a/test_widget.py", "a/widget_test.py"],
            "a/widget.py",
        ),
        (
            &["a/widget.py", "a/test_widget.py", "b/test_widget.py"],
            "a/widget.py",
        ),
        (
            &["a/widget.ts", "a/widget.test.ts", "a/widget.spec.ts"],
            "a/widget.ts",
        ),
        (
            &["src/gate/affected.rs", "tests/gate/affected.rs"],
            "src/gate/affected.rs",
        ),
    ];
    for (paths, changed) in cases {
        assert!(affected_paths(paths, changed).is_empty(), "{paths:?}");
    }
}

#[test]
fn test_runner_command_by_language_and_nearest_manifest() {
    use ripwire_rs::gate::runners::nearest_manifest;
    use std::collections::HashMap;
    let cases: [(&str, &str, Option<&str>, Option<&str>); 10] = [
        (
            "jobs/test_main.py",
            "if __name__ == \"__main__\":\n    pass\n",
            None,
            Some("python3 jobs/test_main.py"),
        ),
        (
            "jobs/test_main.py",
            "if __name__ == '__main__':\n    pass\n",
            None,
            Some("python3 jobs/test_main.py"),
        ),
        (
            "scripts/test_lint.py",
            "#!/usr/bin/env python3\nassert 1\n",
            None,
            None,
        ),
        (
            "test/run_test.sh",
            "echo ok\n",
            None,
            Some("bash test/run_test.sh"),
        ),
        ("test/run_test.bash", "echo ok\n", None, None),
        (
            "pkg/a.test.ts",
            "test('x',()=>{})",
            Some("{\"devDependencies\":{\"vitest\":\"1\"}}"),
            Some("npx vitest run pkg/a.test.ts"),
        ),
        (
            "pkg/a.test.ts",
            "test('x',()=>{})",
            Some("{\"devDependencies\":{\"jest\":\"1\"}}"),
            Some("npx jest pkg/a.test.ts"),
        ),
        (
            "pkg/a.test.js",
            "const t = require('node:test');",
            None,
            Some("node --test pkg/a.test.js"),
        ),
        (
            "pkg/a.test.ts",
            "const t = require('node:test');",
            None,
            None,
        ),
        ("tests/foo_test.go", "package x", None, None),
    ];
    for (path, src, manifest, want) in cases {
        let lang = Lang::from_path(std::path::Path::new(path));
        assert_eq!(
            lang.semantics()
                .test_run_command(path, src, manifest)
                .as_deref(),
            want,
            "{path}"
        );
    }
    let indexed: HashMap<&str, Lang> = [
        ("package.json", Lang::Json),
        ("packages/a/package.json", Lang::Json),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        nearest_manifest("packages/a/test/w.test.ts", "package.json", &indexed),
        Some("packages/a/package.json")
    );
    assert_eq!(
        nearest_manifest("packages/b/test/w.test.ts", "package.json", &indexed),
        Some("package.json")
    );
}

#[test]
fn test_summarize_test_gate_separates_modscope_and_sorts_by_ccx() {
    use ripwire_rs::gate::summarize_test_gate;
    use ripwire_rs::navigate::compute_tested_node_mask;
    let interner = Interner::new();
    let registry = ripwire_rs::LanguageRegistry::new().unwrap();
    let ingest = ripwire_rs::ingest_in_memory_files(
        &[
            ("src/core.py", Lang::Python, b"def leaf():\n    return 1\n"),
            (
                "src/app.py",
                Lang::Python,
                b"def simple():\n    return leaf()\ndef branchy(a, b):\n    if a:\n        if b:\n            return leaf()\n    return 0\nleaf()\n",
            ),
            ("tests/test_core.py", Lang::Python, b"def test_it():\n    simple()\n"),
        ],
        &registry,
        &interner,
    );
    let graph = Graph::build(&ingest, &interner);
    let report = find_affected_tests(&["src/core.py"], &ingest, &graph, &interner);
    let tested = compute_tested_node_mask(&ingest, &graph, &interner);
    let sum = summarize_test_gate(&report, &tested, &ingest, &interner);
    assert_eq!(sum.untested_modscope, 1);
    let names: Vec<&str> = sum
        .untested
        .iter()
        .map(|&id| interner.resolve(&ingest[id].name))
        .collect();
    assert_eq!(names, ["branchy"]);
    assert_eq!(sum.run_first, 1);
}
