use ripwire_rs::ingest::yaml_guard::yaml_nests_too_deep;

#[test]
fn test_yaml_guard_shallow() {
    let simple = b"root:\n  child:\n    grandchild: 1\n";
    assert!(!yaml_nests_too_deep(simple));
}

#[test]
fn test_yaml_guard_deep() {
    let mut deep = Vec::new();
    for i in 0..70 {
        for _ in 0..i {
            deep.extend_from_slice(b"  ");
        }
        deep.extend_from_slice(b"level:\n");
    }
    assert!(yaml_nests_too_deep(&deep));
}
