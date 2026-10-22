use ripwire_rs::cache::{DirtyEntry, dirty_key_suffix};
use std::time::{Duration, UNIX_EPOCH};

fn entry(path: &str, secs: u64, len: u64) -> DirtyEntry {
    (
        path.to_string(),
        Some((UNIX_EPOCH + Duration::from_secs(secs), len)),
    )
}

#[test]
fn clean_tree_adds_nothing_to_the_key() {
    assert_eq!(dirty_key_suffix(&[]), "");
}

#[test]
fn an_edit_to_a_dirty_file_changes_the_key() {
    let before = dirty_key_suffix(&[entry("src/a.ts", 100, 40)]);
    assert_ne!(before, "");
    assert_ne!(before, dirty_key_suffix(&[entry("src/a.ts", 101, 40)]));
    assert_ne!(before, dirty_key_suffix(&[entry("src/a.ts", 100, 41)]));
}

#[test]
fn which_files_are_dirty_changes_the_key() {
    let one = dirty_key_suffix(&[entry("src/a.ts", 100, 40)]);
    let two = dirty_key_suffix(&[entry("src/a.ts", 100, 40), entry("src/b.ts", 100, 10)]);
    assert_ne!(one, two);
    assert_ne!(
        one,
        dirty_key_suffix(&[entry("src/b.ts", 100, 40)]),
        "the same stat on another path is a different tree"
    );
}

#[test]
fn a_deleted_dirty_file_is_part_of_the_key() {
    let gone: DirtyEntry = ("src/a.ts".to_string(), None);
    assert_ne!(
        dirty_key_suffix(&[gone]),
        dirty_key_suffix(&[entry("src/a.ts", 100, 40)])
    );
}

#[test]
fn identical_dirty_state_gives_a_stable_key() {
    let a = dirty_key_suffix(&[entry("src/a.ts", 100, 40)]);
    assert_eq!(a, dirty_key_suffix(&[entry("src/a.ts", 100, 40)]));
}
