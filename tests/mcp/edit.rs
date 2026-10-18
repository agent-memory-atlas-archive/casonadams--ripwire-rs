use ripwire_rs::mcp::edit::{
    EditOp, execute_edit_plan_json, perform_atomic_edit, splice_file_bytes,
};
use ripwire_rs::{FileInfo, IngestResult, Interner, Lang, SymKind, Symbol};

#[test]
fn test_splice_file_bytes_replace() {
    let original = b"fn old_function() { return 1; }\n";
    let payload = b"fn new_function() { return 2; }";
    let (out, new_start, new_end) =
        splice_file_bytes(original, EditOp::ReplaceBody, 0, 31, payload).unwrap();
    assert_eq!(new_start, 0);
    assert_eq!(new_end, payload.len());
    assert_eq!(out, b"fn new_function() { return 2; }\n");
}

#[test]
fn test_splice_file_bytes_insert_before() {
    let original = b"fn foo() {}\n";
    let payload = b"// comment\n";
    let (out, new_start, new_end) =
        splice_file_bytes(original, EditOp::InsertBefore, 0, 11, payload).unwrap();
    assert_eq!(new_start, 0);
    assert_eq!(new_end, payload.len());
    assert_eq!(out, b"// comment\nfn foo() {}\n");
}

#[test]
fn test_splice_file_bytes_insert_after() {
    let original = b"fn foo() {}\n";
    let payload = b"\nfn bar() {}\n";
    let (out, new_start, new_end) =
        splice_file_bytes(original, EditOp::InsertAfter, 0, 11, payload).unwrap();
    assert_eq!(new_start, 11);
    assert_eq!(new_end, 11 + payload.len());
    assert_eq!(out, b"fn foo() {}\nfn bar() {}\n\n");
}

#[test]
fn test_perform_atomic_edit_refusals() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();

    let res_nul = perform_atomic_edit(
        &ingest,
        &interner,
        EditOp::ReplaceBody,
        "sym",
        None,
        "fn foo\0() {}",
    );
    assert_eq!(res_nul["applied"], false);

    let res_missing = perform_atomic_edit(
        &ingest,
        &interner,
        EditOp::ReplaceBody,
        "missing_symbol",
        None,
        "fn foo() {}",
    );
    assert_eq!(res_missing["applied"], false);

    ingest.files.push(FileInfo {
        id: 0,
        path: interner.intern("nonexistent_file_xyz.rs"),
        lang: Lang::Rust,
        line_count: 10,
        byte_size: 100,
    });
    ingest.symbols.push(Symbol::new(
        0,
        interner.intern("found_sym"),
        interner.intern("found_sym"),
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    ));
    let res_unreadable = perform_atomic_edit(
        &ingest,
        &interner,
        EditOp::ReplaceBody,
        "found_sym",
        None,
        "fn found_sym() {}",
    );
    assert_eq!(res_unreadable["applied"], false);

    let bad_json = execute_edit_plan_json(".", "not json", true, &ingest, &interner);
    assert_eq!(bad_json["applied"], false);
    let empty_plan = execute_edit_plan_json(".", "{\"version\":1}", true, &ingest, &interner);
    assert_eq!(empty_plan["applied"], false);
    let missing_sym_plan = execute_edit_plan_json(
        ".",
        "{\"version\":1,\"edits\":[{\"op\":\"replace\",\"symbol\":\"missing\",\"new_body\":\"fn x(){}\"}]}",
        true,
        &ingest,
        &interner,
    );
    assert_eq!(missing_sym_plan["applied"], false);
}
