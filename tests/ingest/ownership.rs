use ripwire_rs::{
    Graph, IngestResult, Interner, Lang, LanguageRegistry, MODULE_SCOPE_NAME, NO_NODE,
    PageRankConfig, SymKind, find_affected_tests, ingest_in_memory_files, navigate_1hop,
    serialize_callers_xml,
};

fn ingest(files: &[(&str, Lang, &[u8])]) -> (IngestResult, Interner) {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let ingest = ingest_in_memory_files(files, &registry, &interner);
    (ingest, interner)
}

fn sym_id(ingest: &IngestResult, interner: &Interner, name: &str) -> u32 {
    ingest
        .symbols
        .iter()
        .find(|s| interner.resolve(&s.name) == name)
        .unwrap_or_else(|| panic!("symbol {name}"))
        .id
}

#[test]
fn top_level_calls_get_a_file_scope_caller() {
    let cases: &[(&str, Lang, &[u8])] = &[
        (
            "scripts/lint.py",
            Lang::Python,
            b"def check(x):\n    return x\n\nfor p in [1, 2]:\n    check(p)\n",
        ),
        (
            "web/app.js",
            Lang::JavaScript,
            b"function check(x) { return x; }\n[1, 2].forEach((p) => check(p));\n",
        ),
    ];
    for &(path, lang, src) in cases {
        let (ingest, interner) = ingest(&[(path, lang, src)]);
        let graph = Graph::build(&ingest, &interner);
        let ranks = graph.page_rank(&PageRankConfig::default()).ranks;
        let check = sym_id(&ingest, &interner, "check");

        let nav = navigate_1hop(check, &ingest, &graph, &ranks, &interner).unwrap();
        assert_eq!(nav.callers.len(), 1, "{path}");
        assert_eq!(nav.callers[0].name, MODULE_SCOPE_NAME, "{path}");
        assert_eq!(nav.callers[0].kind, SymKind::ModuleScope.tag(), "{path}");
        assert_eq!(nav.callers[0].path, path);
        assert!(ingest.references.iter().all(|r| r.from_symbol != NO_NODE));
        assert_eq!(ingest.symbol_count(), ingest.symbols.len() - 1, "{path}");

        let mut xml = Vec::new();
        serialize_callers_xml("check", &nav.callers, &mut xml).unwrap();
        let xml = String::from_utf8(xml).unwrap();
        assert!(
            xml.contains(&format!(
                "t=\"modscope\" n=\"&lt;file-scope&gt;\" p=\"{path}:1\""
            )),
            "{xml}"
        );
    }
}

#[test]
fn file_with_only_definitions_gets_no_owner() {
    let (ingest, _) = ingest(&[(
        "lib/util.py",
        Lang::Python,
        b"def a():\n    b()\n\ndef b():\n    return 1\n",
    )]);
    assert!(
        ingest
            .symbols
            .iter()
            .all(|s| s.kind != SymKind::ModuleScope)
    );
}

#[test]
fn top_level_import_is_not_a_caller() {
    let (ingest, interner) = ingest(&[
        (
            "src/util/Square.kt",
            Lang::Kotlin,
            b"package com.example.util\nfun square(x: Int): Int = x * x\n",
        ),
        (
            "src/Greeter.kt",
            Lang::Kotlin,
            b"package com.example\nimport com.example.util.square\nfun greet(): Int = square(2)\n",
        ),
    ]);
    let graph = Graph::build(&ingest, &interner);
    let ranks = graph.page_rank(&PageRankConfig::default()).ranks;
    let square = sym_id(&ingest, &interner, "square");
    let nav = navigate_1hop(square, &ingest, &graph, &ranks, &interner).unwrap();
    let callers: Vec<&str> = nav.callers.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(callers, ["greet"]);
}

#[test]
fn test_script_calling_at_module_scope_is_affected() {
    let (ingest, interner) = ingest(&[
        (
            "scripts/lint.py",
            Lang::Python,
            b"def check(x):\n    return x\n",
        ),
        (
            "scripts/test_lint.py",
            Lang::Python,
            b"from lint import check\nassert check(1) == 1\n",
        ),
    ]);
    let graph = Graph::build(&ingest, &interner);
    let report = find_affected_tests(&["scripts/lint.py"], &ingest, &graph, &interner);
    let tests: Vec<(&str, u32)> = report
        .tests
        .iter()
        .map(|t| (t.path.as_str(), t.hops))
        .collect();
    assert_eq!(tests, [("scripts/test_lint.py", 1)]);
}
