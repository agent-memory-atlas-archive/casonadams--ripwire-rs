use crate::domain::Lang;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub const DEFAULT_MAX_FILE_SIZE: u64 = 4 * 1024 * 1024;
pub const DENYLIST_DIRS: &[&str] = &[
    ".git",
    ".claude",
    ".hg",
    ".svn",
    "node_modules",
    "vendor",
    "third_party",
    ".cache",
    "build",
    "dist",
    "out",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    ".idea",
    ".vscode",
    "asan",
    "build_prof",
    "CMakeFiles",
    "captures",
    ".specs",
    ".agents",
    ".skills",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawledFile {
    pub path: PathBuf,
    pub rel_path: String,
    pub lang: Lang,
    pub byte_size: u64,
}

#[derive(Debug, Clone)]
pub struct CrawlerOptions {
    pub max_file_size: u64,
    pub honor_gitignore: bool,
    pub custom_excludes: Vec<String>,
}

impl Default for CrawlerOptions {
    fn default() -> Self {
        Self {
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            honor_gitignore: true,
            custom_excludes: Vec::new(),
        }
    }
}

fn build_walker(root: &Path, options: &CrawlerOptions) -> ignore::Walk {
    let custom_excludes = options.custom_excludes.clone();
    let mut builder = WalkBuilder::new(root);
    builder
        .git_ignore(options.honor_gitignore)
        .git_global(options.honor_gitignore)
        .git_exclude(options.honor_gitignore)
        .hidden(false)
        .follow_links(false)
        .filter_entry(move |entry| {
            let Some(name) = entry.file_name().to_str() else {
                return true;
            };
            let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());
            !is_entry_denylisted(name, is_dir, &custom_excludes)
        })
        .build()
}

fn resolve_path_lang(path: &Path) -> Lang {
    let file_name = path.file_name().and_then(|e| e.to_str()).unwrap_or("");
    if file_name.eq_ignore_ascii_case("dockerfile")
        || file_name.eq_ignore_ascii_case("containerfile")
    {
        Lang::Dockerfile
    } else {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        Lang::from_extension(ext)
    }
}

fn process_crawl_entry(
    entry: &ignore::DirEntry,
    root: &Path,
    options: &CrawlerOptions,
) -> Option<CrawledFile> {
    let path = entry.path();
    if path.is_dir() {
        return None;
    }
    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
    let rel_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut file = evaluate_candidate(&rel_path, size, false, options)?;
    file.path = path.to_path_buf();
    Some(file)
}

pub fn crawl_directory(root: &Path, options: &CrawlerOptions) -> Vec<CrawledFile> {
    let mut files = Vec::new();
    for entry in build_walker(root, options).flatten() {
        if let Some(file) = process_crawl_entry(&entry, root, options) {
            files.push(file);
        }
    }
    files.sort_by(|a, b| a.rel_path.as_bytes().cmp(b.rel_path.as_bytes()));
    files
}

#[inline]
fn is_entry_denylisted(name: &str, is_dir: bool, custom_excludes: &[String]) -> bool {
    if is_dir {
        DENYLIST_DIRS.contains(&name)
            || name.starts_with("cmake-build-")
            || custom_excludes.iter().any(|ex| name.contains(ex))
    } else {
        custom_excludes.iter().any(|ex| name.contains(ex))
    }
}

/// Check if a path component is in the default or custom denylist.
#[must_use]
pub fn is_path_denylisted(path: &Path, custom_excludes: &[String]) -> bool {
    path.components().any(|c| {
        c.as_os_str().to_str().is_some_and(|name| {
            DENYLIST_DIRS.contains(&name)
                || name.starts_with("cmake-build-")
                || custom_excludes.iter().any(|ex| name.contains(ex))
        })
    })
}

/// Check if a buffer contains null bytes (binary file heuristic).
#[inline]
#[must_use]
pub fn is_binary_buffer(buf: &[u8]) -> bool {
    buf.contains(&0)
}

/// Pure in-memory evaluation of a candidate file path, language, and size.
#[must_use]
pub fn evaluate_candidate(
    rel_path: &str,
    byte_size: u64,
    is_binary: bool,
    options: &CrawlerOptions,
) -> Option<CrawledFile> {
    let path = Path::new(rel_path);
    if is_path_denylisted(path, &options.custom_excludes) {
        return None;
    }
    let lang = resolve_path_lang(path);
    if lang == Lang::Unknown || byte_size == 0 || byte_size > options.max_file_size || is_binary {
        return None;
    }
    Some(CrawledFile {
        path: path.to_path_buf(),
        rel_path: rel_path.replace('\\', "/"),
        lang,
        byte_size,
    })
}

#[derive(Debug, Default, Clone)]
pub struct SkippedCensus {
    pub oversize: Vec<(String, u64, String)>,
    pub unsupported: Vec<(String, u64, String)>,
    pub excluded: Vec<(String, u64, String)>,
    pub unindexed_exts: Vec<(String, usize)>,
}

fn classify_skipped_entry(
    entry: &ignore::DirEntry,
    root: &Path,
    options: &CrawlerOptions,
    census: &mut SkippedCensus,
    ext_counts: &mut std::collections::HashMap<String, usize>,
) {
    let path = entry.path();
    if path.is_dir() {
        return;
    }
    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
    let rel_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if is_path_denylisted(Path::new(&rel_path), &options.custom_excludes) {
        census.excluded.push((rel_path, size, ext));
        return;
    }
    let lang = resolve_path_lang(path);
    if lang == Lang::Unknown {
        if !ext.is_empty() {
            *ext_counts.entry(ext.clone()).or_default() += 1;
        }
        census.unsupported.push((rel_path, size, ext));
    } else if size > options.max_file_size {
        census.oversize.push((rel_path, size, ext));
    }
}

#[must_use]
pub fn scan_skipped_census(root: &Path, options: &CrawlerOptions) -> SkippedCensus {
    let mut census = SkippedCensus::default();
    let mut ext_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for entry in build_walker(root, options).flatten() {
        classify_skipped_entry(&entry, root, options, &mut census, &mut ext_counts);
    }
    census.oversize.sort_by(|a, b| a.0.cmp(&b.0));
    census.unsupported.sort_by(|a, b| a.0.cmp(&b.0));
    census.excluded.sort_by(|a, b| a.0.cmp(&b.0));
    let mut exts: Vec<(String, usize)> = ext_counts.into_iter().collect();
    exts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    census.unindexed_exts = exts;
    census
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crawl_deterministic_and_denylist() {
        let candidates = [
            ("src/b.rs", 10, false),
            ("src/a.rs", 10, false),
            ("target/ign.rs", 10, false),
            (".git/config", 50, false),
            ("src/unknown.xyz", 10, false),
            ("binary.rs", 10, true),
            ("empty.rs", 0, false),
            ("Dockerfile", 20, false),
        ];

        let opts = CrawlerOptions::default();
        let mut files: Vec<CrawledFile> = candidates
            .iter()
            .filter_map(|&(rel_path, size, is_bin)| {
                evaluate_candidate(rel_path, size, is_bin, &opts)
            })
            .collect();

        files.sort_by(|a, b| a.rel_path.as_bytes().cmp(b.rel_path.as_bytes()));

        assert_eq!(files.len(), 3);
        assert_eq!(files[0].rel_path, "Dockerfile");
        assert_eq!(files[0].lang, Lang::Dockerfile);
        assert_eq!(files[1].rel_path, "src/a.rs");
        assert_eq!(files[2].rel_path, "src/b.rs");
        assert_eq!(files[1].lang, Lang::Rust);
        assert!(is_binary_buffer(b"\0hello"));
        assert!(!is_binary_buffer(b"hello world"));
    }
}
