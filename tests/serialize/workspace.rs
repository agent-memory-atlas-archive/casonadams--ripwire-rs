use ripwire_rs::serialize_whereis_xml;
use serde_json::json;

#[test]
fn worktree_definitions_keep_distinct_locations_when_branch_rows_fold() {
    let value = json!({
        "symbol": "run",
        "listing": "defs",
        "hits": [
            {"ref": "worktree", "on_head": true, "kind": "def", "path": "worker.py", "line": 2, "text": "def run(self):"},
            {"ref": "worktree", "on_head": true, "kind": "def", "path": "worker.py", "line": 8, "text": "def run(self):"},
            {"ref": "feature-a", "on_head": false, "kind": "def", "path": "worker.py", "line": 2, "text": "def run(self):"},
            {"ref": "feature-b", "on_head": false, "kind": "def", "path": "worker.py", "line": 2, "text": "def run(self):"}
        ]
    });
    let mut output = Vec::new();
    serialize_whereis_xml(&value, &mut output).unwrap();
    let xml = String::from_utf8(output).unwrap();
    assert_eq!(xml.matches("<hit ").count(), 3, "{xml}");
    assert!(xml.contains("l=\"8\""), "{xml}");
    assert!(xml.contains("folded=\"1\""), "{xml}");
    assert!(xml.contains("refs=\"2\""), "{xml}");
}
