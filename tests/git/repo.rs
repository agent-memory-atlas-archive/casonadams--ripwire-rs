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

fn blob(c: char) -> gix::ObjectId {
    gix::ObjectId::from_hex(c.to_string().repeat(40).as_bytes()).unwrap()
}

fn tree(entries: &[(&str, char)]) -> std::collections::BTreeMap<String, gix::ObjectId> {
    entries
        .iter()
        .map(|(path, c)| ((*path).to_string(), blob(*c)))
        .collect()
}

fn dirty(paths: &[&str]) -> std::collections::BTreeSet<String> {
    paths.iter().map(|p| (*p).to_string()).collect()
}

#[test]
fn test_changed_paths_reports_committed_change_since_base() {
    use ripwire_rs::git::repo::changed_paths;
    let base = tree(&[("src/a.rs", 'a'), ("src/b.rs", 'b')]);
    let head = tree(&[("src/a.rs", 'c'), ("src/b.rs", 'b')]);
    let changed = changed_paths(&base, &head, &dirty(&[]), |_| None);
    assert_eq!(changed, vec!["src/a.rs"]);
}

#[test]
fn test_changed_paths_empty_when_tree_matches_base() {
    use ripwire_rs::git::repo::changed_paths;
    let base = tree(&[("src/a.rs", 'a')]);
    let changed = changed_paths(&base, &base, &dirty(&[]), |_| None);
    assert!(changed.is_empty());
}

#[test]
fn test_changed_paths_dirty_edit_is_hashed_against_base() {
    use ripwire_rs::git::repo::changed_paths;
    let base = tree(&[("src/a.rs", 'a')]);
    let head = tree(&[("src/a.rs", 'a')]);
    let changed = changed_paths(&base, &head, &dirty(&["src/a.rs"]), |_| Some(blob('d')));
    assert_eq!(changed, vec!["src/a.rs"]);
}

#[test]
fn test_changed_paths_dirty_file_restored_to_base_is_unchanged() {
    use ripwire_rs::git::repo::changed_paths;
    let base = tree(&[("src/a.rs", 'a')]);
    let head = tree(&[("src/a.rs", 'c')]);
    let changed = changed_paths(&base, &head, &dirty(&["src/a.rs"]), |_| Some(blob('a')));
    assert!(changed.is_empty());
}

#[test]
fn test_changed_paths_untracked_and_deleted_files() {
    use ripwire_rs::git::repo::changed_paths;
    let base = tree(&[("src/gone.rs", 'e')]);
    let head = tree(&[("src/gone.rs", 'e')]);
    let changed = changed_paths(
        &base,
        &head,
        &dirty(&["src/gone.rs", "src/new.rs"]),
        |path| (path == "src/new.rs").then(|| blob('f')),
    );
    assert_eq!(changed, vec!["src/gone.rs", "src/new.rs"]);
}

#[test]
fn test_relative_prefix_is_dir_with_trailing_slash_or_empty_at_top() {
    use ripwire_rs::git::repo::relative_prefix;
    use std::path::Path;
    let work = Path::new("/work/tree");
    assert_eq!(relative_prefix(work, work).as_deref(), Some(""));
    assert_eq!(
        relative_prefix(work, Path::new("/work/tree/src/git")).as_deref(),
        Some("src/git/")
    );
    assert_eq!(relative_prefix(work, Path::new("/work/other")), None);
    assert_eq!(relative_prefix(work, Path::new("/work/treeish")), None);
}
