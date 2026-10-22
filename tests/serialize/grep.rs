use ripwire_rs::serialize::serialize_grep_xml;
use serde_json::{Value, json};

fn render(val: &Value) -> String {
    let mut buf = Vec::new();
    serialize_grep_xml(val, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

fn hit(path: &str, line: u64, enclosing: &str) -> Value {
    json!({"path": path, "line": line, "text": "x", "enclosing": enclosing, "enc_id": 1})
}

#[test]
fn test_enc_row_lists_read_sites_for_a_carrier() {
    let xml = render(&json!({
        "pattern": "LIMIT", "verb": "grep", "literal": true,
        "hits": [hit("a.py", 1, "LIMIT")],
        "enc": {"LIMIT": {"callers": 0, "floor": true, "floor_next": "--uses=LIMIT", "cx": 0,
                          "reads": 2, "reads_at": ["a.py:5", "a.py:8"]}},
    }));
    assert!(xml.contains(
        "<enc n=\"LIMIT\" callers=\"0\" callers_floor=\"1\" floor_next=\"--uses=LIMIT\" reads=\"2\" reads_at=\"a.py:5 a.py:8\"/>"
    ));
    assert!(xml.contains("next=\"--uses=LIMIT\""));
}

#[test]
fn test_enc_row_omits_reads_when_none_are_indexed() {
    let xml = render(&json!({
        "pattern": "f", "verb": "grep", "literal": true,
        "hits": [hit("a.py", 1, "f")],
        "enc": {"f": {"callers": 1, "floor": false, "cx": 2, "reads": 0, "reads_at": []}},
    }));
    assert!(xml.contains("<enc n=\"f\" callers=\"1\" cx=\"2\"/>"));
    assert!(!xml.contains("reads"));
    assert!(xml.contains("next=\"--at=a.py:1\""));
}

#[test]
fn test_paging_discloses_window_and_pasteable_next() {
    let hits: Vec<Value> = (1..=5).map(|l| hit("a.py", l, "f")).collect();
    let xml = render(&json!({
        "pattern": "p", "verb": "grep", "literal": true, "limit": 2, "offset": 1,
        "hits": hits, "enc": {"f": {"callers": 0, "floor": false, "cx": 1}},
    }));
    assert!(xml.contains("hits=\"5\" shown=\"2\" capped=\"1\" total=\"5\" has_more=\"1\" next_offset=\"3\" offset=\"1\" limit=\"2\""));
    assert!(xml.contains("next=\"--grep=p --offset=3 --legend=compact\""));
    assert!(!xml.contains("complete=\"1\""));
    assert!(
        xml.contains("<hit l=\"2\"")
            && xml.contains("<hit l=\"3\"")
            && !xml.contains("<hit l=\"4\"")
    );
}

#[test]
fn test_budget_windows_hits_when_no_limit_is_given() {
    let hits: Vec<Value> = (1..=50).map(|l| hit("a.py", l, "f")).collect();
    let xml = render(&json!({
        "pattern": "p", "verb": "grep", "literal": true, "budget": 1,
        "hits": hits, "enc": {"f": {"callers": 0, "floor": false, "cx": 1}},
    }));
    assert!(xml.contains("hits=\"50\" shown=\"1\" capped=\"1\" total=\"50\" has_more=\"1\""));
    assert!(xml.contains("hits_capped=\"1\""));
    assert!(xml.contains("<hit l=\"1\""));
    assert!(!xml.contains("<hit l=\"2\""));
    assert!(xml.contains("next=\"--grep=p --offset=1 --legend=compact\""));
}

#[test]
fn test_large_budget_shows_every_hit() {
    let hits: Vec<Value> = (1..=5).map(|l| hit("a.py", l, "f")).collect();
    let xml = render(&json!({
        "pattern": "p", "verb": "grep", "literal": true, "budget": 1000000,
        "hits": hits, "enc": {"f": {"callers": 0, "floor": false, "cx": 1}},
    }));
    assert!(xml.contains("hits=\"5\" shown=\"5\" capped=\"0\" hits_capped=\"0\""));
    assert_eq!(xml.matches("<hit ").count(), 5);
    assert!(xml.contains("hits_capped=\"0\""));
    assert!(xml.contains("complete=\"1\""));
}

#[test]
fn test_explicit_limit_wins_over_budget() {
    let hits: Vec<Value> = (1..=5).map(|l| hit("a.py", l, "f")).collect();
    let xml = render(&json!({
        "pattern": "p", "verb": "grep", "literal": true, "limit": 3, "budget": 1,
        "hits": hits, "enc": {"f": {"callers": 0, "floor": false, "cx": 1}},
    }));
    assert!(xml.contains("hits=\"5\" shown=\"3\" capped=\"1\" total=\"5\" has_more=\"1\""));
    assert!(xml.contains("<hit l=\"3\""));
    assert!(!xml.contains("<hit l=\"4\""));
}

#[test]
fn test_identical_sites_merge_into_one_hit_with_at_markers() {
    let xml = render(&json!({
        "pattern": "p", "verb": "grep", "literal": true,
        "hits": [hit("a.py", 5, "f"), hit("a.py", 5, "f"), hit("a.py", 5, "f")],
        "enc": {"f": {"callers": 0, "floor": false, "cx": 1}},
    }));
    assert!(xml.contains("<hit l=\"5\" in=\"f\" n=\"3\">"));
    assert_eq!(xml.matches("<at l=\"5\" in=\"f\"/>").count(), 2);
    assert!(xml.contains("hits=\"3\""));
}

#[test]
fn test_unindexed_hits_are_counted_apart_and_listed_in_trailing_element() {
    let xml = render(&json!({
        "pattern": "p", "verb": "grep", "literal": true,
        "hits": [hit("a.py", 1, "f")],
        "enc": {"f": {"callers": 0, "floor": false, "cx": 1}},
        "unindexed": [{"path": "n.txt", "line": 1, "text": "p"}],
        "unindexed_files_scanned": 3, "corpus_pruned_dirs": 2,
    }));
    assert!(xml.contains("hits=\"1\""));
    assert!(
        xml.contains("corpus_pruned_dirs=\"2\" unindexed_hits=\"1\" unindexed_files_scanned=\"3\"")
    );
    assert!(xml.contains("<unindexed count=\"1\" shown=\"1\" capped=\"0\">"));
    assert!(xml.contains("<f p=\"n.txt\">"));
    assert!(xml.contains("complete=\"1\""));
}

#[test]
fn test_suppressed_tiers_withdraw_the_complete_claim() {
    let xml = render(&json!({
        "pattern": "p", "verb": "grep", "literal": true, "suppressed_comment": 2,
        "hits": [hit("a.py", 1, "f")],
        "enc": {"f": {"callers": 0, "floor": false, "cx": 1}},
    }));
    assert!(xml.contains("suppressed_comment=\"2\""));
    assert!(!xml.contains("complete=\"1\""));
}
