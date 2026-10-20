use ripwire_rs::git::history::CommitInfo;
use ripwire_rs::mcp::doc_tools::{
    call_mentions_in_root, call_owners, call_stray_content, call_whereis, classify_code_line,
    collect_file_wanted_tokens, conditional_uses, enrich_drift_item_history, extract_cmake_option,
    extract_env_identifiers, extract_settings_gate, filter_present_in_code_drift,
    inspect_branch_stray, scan_doc_text_for_drift, should_scan_branch,
};
use ripwire_rs::{
    FileInfo, IngestResult, Interner, Lang, SymKind, Symbol, serialize_doc_drift_xml,
};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[test]
fn test_mentions_empty() {
    let ingest = IngestResult::new();
    let interner = Interner::new();
    let res = call_mentions_in_root(".", &ingest, &interner, "nonexistent");
    assert_eq!(res["total_mentions"], 0);
}

#[test]
fn test_owners_attribution() {
    let commits = vec![
        CommitInfo {
            id: "c1".to_string(),
            timestamp: 1000,
            author: "alice@example.com".to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
        },
        CommitInfo {
            id: "c2".to_string(),
            timestamp: 2000,
            author: "bob@example.com".to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
        },
    ];
    let res = call_owners(Some(&commits), None, None, None);
    assert_eq!(res["files_analyzed"], 1);
    let files = res["files"].as_array().unwrap();
    assert_eq!(files[0]["authors"], 2);
}

#[test]
fn test_env_identifier_extraction_table() {
    let cases: &[(&str, &str, &[&str])] = &[
        ("a.c", "getenv(\"BAR\")", &["BAR"]),
        ("a.cpp", "std::getenv(\"CPP1\")", &["CPP1"]),
        ("a.c", "getenv( \"INSP\" )", &["INSP"]),
        ("a.py", "x = getenv (\"GSP\")", &["GSP"]),
        ("a.py", "os.getenv(\"BAZ\")", &["BAZ"]),
        ("a.py", "os.environ.get(\"QUX\", \"d\")", &["QUX"]),
        ("a.py", "os.environ.get (\"EGS\")", &["EGS"]),
        (
            "a.py",
            "dsn = os.environ[\"DATABASE_URL\"]",
            &["DATABASE_URL"],
        ),
        ("a.py", "f = os.environ[ \"SPC\" ]", &["SPC"]),
        ("a.py", "e = environ[\"BARE\"]", &["BARE"]),
        ("a.py", "x = os.environ[\"lower_case\"]", &["lower_case"]),
        (
            "a.py",
            "h = os.environ[\"X1\"] + os.environ[\"X2\"]",
            &["X1", "X2"],
        ),
        (
            "a.py",
            "a = os.environ[\"DUP\"] + os.environ[\"DUP\"]",
            &["DUP"],
        ),
        (
            "a.py",
            "_os.environ[\"COMMIT_SHA\"] = \"cafebabe\"",
            &["COMMIT_SHA"],
        ),
        (
            "a.py",
            "c = \"it's\" and os.getenv(\"AFTERAPOS\")",
            &["AFTERAPOS"],
        ),
        (
            "a.yml",
            "cfg = json.loads(os.environ[\"CONFIG_JSON\"])",
            &["CONFIG_JSON"],
        ),
        (
            "a.ts",
            "  * docs (injected via `process.env.DOCS_EMBED`)",
            &[],
        ),
        ("a.ts", "/* process.env.BLOCK */", &[]),
        (
            "a.ts",
            "define: { \"process.env.APP_BUNDLED\": JSON.stringify(\"true\") },",
            &[],
        ),
        ("a.js", "process.env.VAR_123", &["VAR_123"]),
        ("a.js", "process.env.A + process.env.B", &["A", "B"]),
        ("a.js", "const a = process.env[\"PDQ\"];", &["PDQ"]),
        ("a.js", "const b = process.env['PSQ'];", &["PSQ"]),
        ("a.js", "const b = process.env[`PBT`];", &["PBT"]),
        ("a.js", "const a = process.env[ \"PSPC\" ];", &["PSPC"]),
        (
            "a.rs",
            "fn f<'a>(x: &'a str) { let c = 'c'; getenv(\"RSGET\") }",
            &["RSGET"],
        ),
        ("a.rs", "let a = std::env::var(\"FOO\");", &[]),
        ("a.py", "b = os.environ['SQ']", &[]),
        ("a.py", "c = os.getenv('GSQ')", &[]),
        ("a.py", "d = os.environ.get('EGSQ')", &[]),
        ("a.py", "i = os.environ  [\"SP2\"]", &[]),
        ("a.py", "c = myenviron[\"MYENV\"]", &[]),
        ("a.py", "g = env[\"PLAINENV\"]", &[]),
        ("a.py", "b = mygetenv(\"MYG\")", &[]),
        ("a.py", "e = os.environ[KEY]", &[]),
        ("a.py", "f = os.environ[\"A-B\"]", &[]),
        ("a.py", "i = os.environ[\"9START\"]", &[]),
        ("a.py", "j = os.environ[\"\"]", &[]),
        ("a.py", "# os.getenv(\"CMT\")", &[]),
        ("a.py", "c = 'os.environ[\"INSQ\"]'", &[]),
        (
            "a.sh",
            "python3 -c 'import os; print(os.environ[\"SHPY\"])'",
            &[],
        ),
        ("a.sh", "echo ${SHV} ${SHD:-x} $SHP", &[]),
        ("a.js", "const b = \"process.env.INSTR\";", &[]),
        ("a.js", "// process.env.CMT", &[]),
        ("a.js", "const c = process.env[name];", &[]),
        ("a.js", "const b = process.env [ \"PSP\" ];", &[]),
        ("a.js", "const c = myprocess.env.MYP;", &[]),
        ("a.js", "const d = Deno.env.get(\"DENO\");", &[]),
        ("a.rs", "let x = 42;", &[]),
    ];
    for (path, line, want) in cases {
        let lang = ripwire_rs::Lang::from_path(std::path::Path::new(path));
        assert_eq!(extract_env_identifiers(line, lang), *want, "{path}: {line}");
    }
}

#[test]
fn test_gate_extractors() {
    let gate = |src: &[&str]| {
        extract_settings_gate(src, 0).map(|g| (g.name, g.default, g.dark, g.define_idx))
    };
    assert_eq!(
        gate(&["#ifndef DARK_GATE", "#define DARK_GATE 0", "int x;"]),
        Some(("DARK_GATE".into(), "0".into(), true, 1))
    );
    assert_eq!(
        gate(&["#ifndef LIT_GATE", "", "#define LIT_GATE 1"]),
        Some(("LIT_GATE".into(), "1".into(), false, 2))
    );
    assert_eq!(gate(&["#ifndef GUARD", "#define GUARD"]), None);
    assert_eq!(
        gate(&["#ifndef MY_HEADER_H", "#define MY_HEADER_H 1"]),
        Some(("MY_HEADER_H".into(), "1".into(), false, 1))
    );
    assert_eq!(
        gate(&["#ifdef SOME_FEATURE", "#define SOME_FEATURE 1"]),
        None
    );
    assert_eq!(gate(&["#ifndef A", "#define B 1"]), None);
    let lines = [
        "#if defined(RW_SIMD) && RW_SIMD",
        "a;",
        "b;",
        "#endif",
        "#ifndef RW_SIMD",
        "#endif",
        "#ifdef RW_TRACE",
        "#elif 1",
        "#endif",
    ];
    let uses: Vec<(String, usize, usize)> = conditional_uses(&lines)
        .into_iter()
        .map(|u| (u.name, u.line_idx, u.loc))
        .collect();
    assert_eq!(
        uses,
        [
            ("RW_SIMD".to_string(), 0, 2),
            ("RW_SIMD".to_string(), 0, 2),
            ("RW_TRACE".to_string(), 6, 1),
        ]
    );
    assert_eq!(
        extract_cmake_option("option(ENABLE_TESTS \"doc\" ON)", "CMakeLists.txt"),
        Some(("ENABLE_TESTS".into(), "ON".into(), false))
    );
    assert_eq!(
        extract_cmake_option("option(ENABLE_TESTS \"doc\" ON)", "main.rs"),
        None
    );
}

#[test]
fn test_branch_stray_inspection() {
    let repo = ripwire_rs::git::repo::discover(".").unwrap();
    assert_eq!(inspect_branch_stray(&repo, "", "main"), None);
    assert_eq!(inspect_branch_stray(&repo, "HEAD", "main"), None);
    assert_eq!(inspect_branch_stray(&repo, "main", "main"), None);
    let (verdict, _) = inspect_branch_stray(&repo, "nonexistent-branch-xyz", "main").unwrap();
    assert_eq!(verdict, "unknown");
    let (base, ahead) = ripwire_rs::git::repo::unmerged_commits(&repo, "HEAD").unwrap();
    assert_eq!(ahead, 0);
    assert_eq!(
        base,
        ripwire_rs::git::repo::short_id(repo.head_id().unwrap())
    );
}

#[test]
fn test_whereis_and_stray() {
    let mut ingest = IngestResult::new();
    let interner = Interner::new();
    let f0 = FileInfo {
        id: 0,
        path: interner.intern("src/main.rs"),
        lang: Lang::Rust,
        line_count: 10,
        byte_size: 100,
    };
    let sym = Symbol::new(
        0,
        interner.intern("main"),
        interner.intern("c_main"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    );
    ingest.files.push(f0);
    ingest.symbols.push(sym);

    let whereis_res = call_whereis(&ingest, &interner, ".", "main", false);
    assert!(whereis_res["count"].as_u64().unwrap_or(0) >= 1);
    let whereis_hist = call_whereis(&ingest, &interner, ".", "nonexistent_sym", true);
    assert!(whereis_hist.get("history").is_some());
    let stray_res = call_stray_content(".");
    assert!(stray_res.get("head").is_some());
}

#[test]
fn test_branch_grep_parsing() {
    assert_eq!(classify_code_line("fn foo()"), "def");
    assert_eq!(classify_code_line("pub fn bar()"), "def");
    assert_eq!(classify_code_line("pub(crate) async fn baz()"), "def");
    assert_eq!(classify_code_line("export async function qux()"), "def");
    assert_eq!(classify_code_line("let x = foo();"), "ref");
    assert!(should_scan_branch("feature-1", true));
    assert!(!should_scan_branch("HEAD", true));
    assert!(!should_scan_branch("", true));
    assert!(!should_scan_branch("main", true));
    assert!(should_scan_branch("main", false));
}

#[test]
fn test_doc_drift_backtick_and_line_anchors() {
    let mut file_map = HashMap::new();
    file_map.insert("src/lib.rs", 20);
    let mut sym_set = HashSet::new();
    sym_set.insert("existingFn");

    let mut ingest = IngestResult::new();
    let interner = Interner::new();
    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern("src/lib.rs"),
        lang: Lang::Rust,
        line_count: 20,
        byte_size: 100,
    });
    let mut sym = Symbol::new(
        0,
        interner.intern("existingFn"),
        interner.intern("src/lib.rs::existingFn"),
        SymKind::Function,
        Lang::Rust,
        0,
        10,
    );
    sym.loc = 3;
    ingest.symbols.push(sym);

    let mut drift = Vec::new();
    let counts = scan_doc_text_for_drift(
        "See `existingFn` at src/lib.rs:3 and `deletedFunction` at src/lib.rs:50 (`--flag` `<s t=\"fn\">` `multi word`)\n```rust\n`ignoredInFence`\n```",
        "README.md",
        &file_map,
        &sym_set,
        &ingest,
        &interner,
        &mut drift,
    );
    assert_eq!(counts.total, 4);
    assert_eq!(counts.checked, 4);
    assert_eq!(drift.len(), 3);
    assert!(
        drift
            .iter()
            .any(|d| d["why"] == "line-moved" && d["tgt"] == "src/lib.rs:10")
    );
    assert!(drift.iter().any(|d| d["referenced"] == "src/lib.rs:50"));
    assert!(drift.iter().any(|d| d["referenced"] == "deletedFunction"));
    filter_present_in_code_drift(Path::new("."), &ingest, &interner, &mut drift);
    assert_eq!(drift.len(), 3);
    let mut wanted = HashSet::new();
    wanted.insert("deletedFunction".to_string());
    let mut wants_first = [false; 256];
    wants_first[b'd' as usize] = true;
    let found =
        collect_file_wanted_tokens(b"fn helper() { deletedFunction(); }", &wants_first, &wanted);
    assert_eq!(found, vec!["deletedFunction".to_string()]);
    enrich_drift_item_history(".", &mut drift[1], "HEAD");
    assert!(drift[1].get("fate").is_some());
    let mut xml_buf = Vec::new();
    serialize_doc_drift_xml(
        &json!({
            "clean_docs": 2,
            "dated_docs": 1,
            "prose_docs": 1,
            "corpus": 10,
            "anchors": 4,
            "checked": 4,
            "unchecked": 0,
            "drift": drift
        }),
        &mut xml_buf,
    )
    .unwrap();
    let xml_str = String::from_utf8(xml_buf).unwrap();
    assert!(xml_str.contains("fate=\""));
    assert!(xml_str.contains("prose=\"1\" corpus=\"10\""));
}
