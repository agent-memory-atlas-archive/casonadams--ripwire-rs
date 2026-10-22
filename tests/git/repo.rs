use ripwire_rs::git::repo::short_id;

#[test]
fn test_short_id_formatting_and_length() {
    let oid = gix::ObjectId::from_hex(b"0123456789abcdef0123456789abcdef01234567").unwrap();
    let sid = short_id(oid);
    assert_eq!(sid, "012345678");
    assert_eq!(sid.len(), 9);

    let zeros = gix::ObjectId::null(gix::hash::Kind::Sha1);
    let zero_sid = short_id(zeros);
    assert_eq!(zero_sid, "000000000");
}

#[test]
fn test_has_word_boundaries_and_identifiers() {
    use ripwire_rs::git::repo::has_word;

    assert!(has_word("hello world", "world"));
    assert!(has_word("world", "world"));
    assert!(has_word("(world);", "world"));
    assert!(has_word("hello world!", "world"));

    // Identifier characters: alphanumeric + '_'
    assert!(!has_word("hello_world", "world"));
    assert!(!has_word("world_hello", "world"));
    assert!(!has_word("worldwide", "world"));
    assert!(!has_word("myworld", "world"));
    assert!(!has_word("world1", "world"));

    // Empty cases
    assert!(!has_word("anything", ""));
    assert!(!has_word("", "world"));
}
