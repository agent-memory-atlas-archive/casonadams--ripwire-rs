//! Changed-file discovery shared by the safety and handoff commands.

pub fn get_working_tree_changed_files(root: &str) -> Vec<String> {
    crate::git::repo::working_tree_changes(root)
}

pub fn resolve_changed_or_spec_files(root: &str, files_spec: Option<&str>) -> Vec<String> {
    match files_spec {
        Some(f) => f
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        None => get_working_tree_changed_files(root),
    }
}
