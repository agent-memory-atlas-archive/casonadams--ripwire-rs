use ripwire_rs::git::history::CommitInfo;
use ripwire_rs::mcp::doc_tools::{
    call_mentions, call_owners, call_stray_content, call_whereis, classify_code_line,
    collect_file_wanted_tokens, count_rev_list, enrich_drift_item_history, extract_cmake_option,
    extract_compile_guard, extract_compile_guard_with_default, extract_env_identifier,
    filter_present_in_code_drift, inspect_branch_stray, parse_branch_grep_line,
    scan_doc_text_for_drift, should_scan_branch,
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
    let res = call_mentions(&ingest, &interner, "nonexistent");
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
fn test_gate_extractors() {
    assert_eq!(
        extract_env_identifier("std::env::var(\"FOO\")"),
        Some("FOO".into())
    );
    assert_eq!(
        extract_env_identifier("getenv(\"BAR\")"),
        Some("BAR".into())
    );
    assert_eq!(
        extract_env_identifier("os.getenv(\"BAZ\")"),
        Some("BAZ".into())
    );
    assert_eq!(
        extract_env_identifier("os.environ.get(\"QUX\")"),
        Some("QUX".into())
    );
    assert_eq!(
        extract_env_identifier("process.env.VAR_123"),
        Some("VAR_123".into())
    );
    assert_eq!(extract_env_identifier("let x = 42;"), None);

    assert_eq!(
        extract_compile_guard("#ifndef MY_FLAG"),
        Some("MY_FLAG".into())
    );
    assert_eq!(
        extract_compile_guard("#ifdef OTHER_FLAG"),
        Some("OTHER_FLAG".into())
    );
    assert_eq!(extract_compile_guard("#include <stdio.h>"), None);
    assert_eq!(extract_compile_guard("#ifndef MY_HEADER_H"), None);
    let lines = ["#ifndef DARK_GATE", "#define DARK_GATE 0", "int x;"];
    assert_eq!(
        extract_compile_guard_with_default(&lines, 0),
        Some(("DARK_GATE".into(), "0".into(), true))
    );
    let lit_lines = ["#ifndef LIT_GATE", "#define LIT_GATE 1"];
    assert_eq!(
        extract_compile_guard_with_default(&lit_lines, 0),
        Some(("LIT_GATE".into(), "1".into(), false))
    );
    let ifdef_lines = ["#ifdef SOME_FEATURE"];
    assert_eq!(
        extract_compile_guard_with_default(&ifdef_lines, 0),
        Some(("SOME_FEATURE".into(), "0".into(), true))
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
    assert_eq!(inspect_branch_stray(".", "", "main"), None);
    assert_eq!(inspect_branch_stray(".", "HEAD", "main"), None);
    assert_eq!(inspect_branch_stray(".", "main", "main"), None);
    let res = inspect_branch_stray(".", "nonexistent-branch-xyz", "main");
    assert!(res.is_some());
    assert_eq!(count_rev_list(".", "HEAD", "HEAD"), 0);
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

    let hit = parse_branch_grep_line(
        "origin/main:src/lib.rs:42:fn hello()",
        "origin/main",
        "hello",
    );
    assert!(hit.is_some());
    let val = hit.unwrap();
    assert_eq!(val["kind"], "def");
    assert_eq!(val["line"], 42);
    assert_eq!(val["file"], "src/lib.rs");
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
