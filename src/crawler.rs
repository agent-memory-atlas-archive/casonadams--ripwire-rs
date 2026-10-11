use crate::domain::Lang;
use ignore::WalkBuilder;
use std::fs::File;
use std::io::Read;
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

pub fn crawl_directory(root: &Path, options: &CrawlerOptions) -> Vec<CrawledFile> {
    let mut builder = WalkBuilder::new(root);
    builder
        .git_ignore(options.honor_gitignore)
        .git_global(options.honor_gitignore)
        .git_exclude(options.honor_gitignore)
        .hidden(false)
        .follow_links(false);

    let mut files = Vec::new();

    for entry in builder.build().flatten() {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }

        let is_denylisted = path.components().any(|c| {
            c.as_os_str().to_str().is_some_and(|name| {
                DENYLIST_DIRS.contains(&name)
                    || name.starts_with("cmake-build-")
                    || options.custom_excludes.iter().any(|ex| name.contains(ex))
            })
        });
        if is_denylisted {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang = Lang::from_extension(ext);
        if lang == Lang::Unknown {
            continue;
        }

        let file_size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if file_size == 0 || file_size > options.max_file_size || is_binary_file(path) {
            continue;
        }

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

    files.sort_by(|a, b| a.rel_path.as_bytes().cmp(b.rel_path.as_bytes()));
    files
}

fn is_binary_file(path: &Path) -> bool {
    let mut buf = [0u8; 4096];
    File::open(path)
        .and_then(|mut f| f.read(&mut buf))
        .map(|n| buf[..n].contains(&0))
        .unwrap_or(false)
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

        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("src/b.rs"), "fn b() {}").unwrap();
        fs::write(root.join("src/a.rs"), "fn a() {}").unwrap();
        fs::write(root.join("target/ign.rs"), "fn ign() {}").unwrap();
        fs::write(root.join("src/unknown.xyz"), "data").unwrap();
        fs::write(root.join("binary.rs"), b"\0").unwrap();

        let files = crawl_directory(root, &CrawlerOptions::default());
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].rel_path, "src/a.rs");
        assert_eq!(files[1].rel_path, "src/b.rs");
        assert_eq!(files[0].lang, Lang::Rust);
    }
}
