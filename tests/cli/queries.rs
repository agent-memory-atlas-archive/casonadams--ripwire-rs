use ripwire_rs::cli::{
    build_handoff_verified_rows, build_handoff_xml, execute_expand_or_outline_query,
    resolve_git_branch_and_subject, sig_has_static_token, skeletonize_body,
};
use ripwire_rs::{IngestResult, Interner, Lang, LanguageRegistry, ingest_in_memory_files};

#[test]
fn test_sig_has_static_token() {
    assert!(sig_has_static_token(b"static void foo()", 0, 17));
    assert!(sig_has_static_token(b"inline static int bar()", 0, 23));
    assert!(sig_has_static_token(b"static", 0, 6));
    assert!(sig_has_static_token(b"foo static bar", 0, 14));
    assert!(!sig_has_static_token(b"mystatic void foo()", 0, 19));
    assert!(!sig_has_static_token(b"static_fn void foo()", 0, 20));
    assert!(!sig_has_static_token(b"void foo()", 0, 10));
    assert!(!sig_has_static_token(b"static void foo()", 10, 5));
    assert!(!sig_has_static_token(b"", 0, 0));
    assert!(sig_has_static_token(b"static", 0, 100));
}

#[test]
fn test_skeletonize_body() {
    let body = "fn run() {\n    if x {\n        let a = 1;\n        let b = 2;\n    }\n}";
    let skel = skeletonize_body(body);
    assert!(skel.contains("..."));
    assert!(!skel.contains("let a = 1"));
    let interner = Interner::new();
    let ingest = IngestResult::new();
    let mut buf = Vec::new();
    assert!(
        execute_expand_or_outline_query(".", "missing", false, &ingest, &interner, &mut buf)
            .is_err()
    );
    assert!(
        execute_expand_or_outline_query(".", "missing", true, &ingest, &interner, &mut buf)
            .is_err()
    );
}

#[test]
fn test_handoff_xml_branches() {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let fixtures: &[(&str, Lang, &[u8])] = &[("src/lib.rs", Lang::Rust, b"pub fn overview() {}\n")];
    let ingest = ingest_in_memory_files(fixtures, &reg, &interner);
    let empty_rows = build_handoff_verified_rows(&[], &ingest, &interner);
    assert!(empty_rows.contains("src/lib.rs"));
    let ch_rows = build_handoff_verified_rows(&["src/lib.rs".to_string()], &ingest, &interner);
    assert!(ch_rows.contains("overview"));
    let xml = build_handoff_xml(".", &ingest, &interner);
    assert!(xml.contains("ripwire-rs.handoff/v1"));
    let (b, s, d) = resolve_git_branch_and_subject("/nonexistent_repo_path_xyz");
    assert_eq!((b.as_str(), s.as_str(), d), ("main", "head", false));
}
