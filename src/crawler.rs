use crate::domain::Lang;
use ignore::WalkBuilder;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Default maximum file size to index (4 MB).
pub const DEFAULT_MAX_FILE_SIZE: u64 = 4 * 1024 * 1024;

/// Denylisted directory names skipped unconditionally.
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
];

/// A file discovered during repository crawl, ready for indexing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawledFile {
    /// Absolute or base-relative path on filesystem.
    pub path: PathBuf,
    /// Normalized relative path (using forward slashes) from crawl root.
    pub rel_path: String,
    /// Detected language based on extension.
    pub lang: Lang,
    /// File size in bytes.
    pub byte_size: u64,
}

/// Configuration options for the directory crawler.
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

/// Crawl a root directory deterministically, returning a lexicographically sorted file list.
pub fn crawl_directory(root: &Path, options: &CrawlerOptions) -> Vec<CrawledFile> {
    let mut builder = WalkBuilder::new(root);
    builder
        .git_ignore(options.honor_gitignore)
        .git_global(options.honor_gitignore)
        .git_exclude(options.honor_gitignore)
        .hidden(false) // Don't skip hidden by default, denylist takes care of .git
        .follow_links(false);

    let walker = builder.build();
    let mut files = Vec::new();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }

        // Check if any path component is in the denylist
        let is_denylisted = path.components().any(|c| {
            if let Some(name) = c.as_os_str().to_str() {
                DENYLIST_DIRS.contains(&name)
                    || name.starts_with("cmake-build-")
                    || options.custom_excludes.iter().any(|ex| name.contains(ex))
            } else {
                false
            }
        });

        if is_denylisted {
            continue;
        }

        // Check file extension
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang = Lang::from_extension(ext);
        if lang == Lang::Unknown {
            continue;
        }

        // Check file size metadata
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let file_size = metadata.len();
        if file_size == 0 || file_size > options.max_file_size {
            continue;
        }

        // Sniff binary content (NUL byte in first 4096 bytes)
        if is_binary_file(path) {
            continue;
        }

        // Normalize relative path
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        files.push(CrawledFile {
            path: path.to_path_buf(),
            rel_path,
            lang,
            byte_size: file_size,
        });
    }

    // Deterministic sort: lexicographical by relative path byte order
    files.sort_by(|a, b| a.rel_path.as_bytes().cmp(b.rel_path.as_bytes()));
    files
}

/// Binary check: inspect first 4096 bytes for NUL byte (`\0`).
fn is_binary_file(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut buffer = [0u8; 4096];
    let bytes_read = match file.read(&mut buffer) {
        Ok(n) => n,
        Err(_) => return false,
    };

    buffer[..bytes_read].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_crawl_deterministic_and_denylist() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        // Create sample files
        let src = root.join("src");
        let target = root.join("target");
        let hidden = root.join(".git");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&hidden).unwrap();

        fs::write(src.join("b.rs"), "fn b() {}").unwrap();
        fs::write(src.join("a.rs"), "fn a() {}").unwrap();
        fs::write(target.join("ignored.rs"), "fn ign() {}").unwrap();
        fs::write(hidden.join("git_file.rs"), "fn git() {}").unwrap();
        fs::write(src.join("unknown.xyz"), "data").unwrap();

        let files = crawl_directory(root, &CrawlerOptions::default());

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].rel_path, "src/a.rs");
        assert_eq!(files[1].rel_path, "src/b.rs");
        assert_eq!(files[0].lang, Lang::Rust);
        assert_eq!(files[1].lang, Lang::Rust);
    }

    #[test]
    fn test_skip_binary_file() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        let bin_path = root.join("binary.rs");
        fs::write(&bin_path, b"fn main() {\0}").unwrap();

        let files = crawl_directory(root, &CrawlerOptions::default());
        assert_eq!(files.len(), 0);
    }
}
