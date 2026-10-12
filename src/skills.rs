use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/embedded_skills.rs"));

/// Report returned after installing embedded agent skills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInstallReport {
    pub installed_count: usize,
    pub destination: PathBuf,
    pub skills: Vec<String>,
}

/// Compute default cross-agent skills directory (~/.agents/skills).
pub fn default_skills_dir() -> PathBuf {
    if let Ok(agents_home) = std::env::var("AGENTS_HOME") {
        return PathBuf::from(agents_home).join("skills");
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".agents").join("skills");
    }

    #[cfg(windows)]
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(user_profile).join(".agents").join("skills");
    }

    PathBuf::from(".agents").join("skills")
}

/// Resolve destination skills directory from optional user CLI argument.
pub fn resolve_skills_dir(custom_path: Option<&str>) -> PathBuf {
    let Some(raw) = custom_path else {
        return default_skills_dir();
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return default_skills_dir();
    }

    let expanded = expand_tilde(trimmed);
    let path = PathBuf::from(expanded);
    if path.file_name().and_then(|n| n.to_str()) == Some(".agents") {
        path.join("skills")
    } else {
        path
    }
}

fn expand_tilde(path_str: &str) -> String {
    if let Some(rest) = path_str.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    } else if path_str == "~"
        && let Ok(home) = std::env::var("HOME")
    {
        return home;
    }
    path_str.to_string()
}

/// Extract unique skill directory names from embedded assets.
pub fn get_skill_names() -> Vec<&'static str> {
    let mut names = BTreeSet::new();
    for (rel_path, _) in EMBEDDED_SKILLS {
        if let Some(first_seg) = rel_path.split('/').next()
            && first_seg.starts_with("ripwire-")
        {
            names.insert(first_seg);
        }
    }
    names.into_iter().collect()
}

/// Generate `.ripwire-manifest-v1` format content.
pub fn generate_manifest_content(skills: &[&str]) -> String {
    let mut out = String::from("version=1\n");
    for skill in skills {
        out.push_str("skill=");
        out.push_str(skill);
        out.push('\n');
    }
    out
}

fn clean_path_if_symlink(path: &Path) -> io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path)
        && meta.file_type().is_symlink()
    {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn write_skill_file(dest_dir: &Path, rel_path: &str, content: &str) -> io::Result<()> {
    let target_file = dest_dir.join(rel_path);
    if let Some(parent) = target_file.parent() {
        clean_path_if_symlink(parent)?;
        fs::create_dir_all(parent)?;
    }
    clean_path_if_symlink(&target_file)?;
    fs::write(&target_file, content)
}
fn write_manifest_file(dest_dir: &Path, skill_names: &[&str]) -> io::Result<()> {
    let manifest_content = generate_manifest_content(skill_names);
    let manifest_path = dest_dir.join(".ripwire-manifest-v1");
    clean_path_if_symlink(&manifest_path)?;
    fs::write(&manifest_path, manifest_content)
}

fn write_all_skill_files(dest_dir: &Path) -> io::Result<()> {
    for (rel_path, content) in EMBEDDED_SKILLS {
        write_skill_file(dest_dir, rel_path, content)?;
    }
    Ok(())
}

/// Install embedded skills and write active manifest to destination directory.
pub fn install_skills(custom_path: Option<&str>) -> io::Result<SkillInstallReport> {
    let dest_dir = resolve_skills_dir(custom_path);
    fs::create_dir_all(&dest_dir)?;
    write_all_skill_files(&dest_dir)?;
    let skill_names = get_skill_names();
    write_manifest_file(&dest_dir, &skill_names)?;

    Ok(SkillInstallReport {
        installed_count: skill_names.len(),
        destination: dest_dir,
        skills: skill_names.into_iter().map(String::from).collect(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_skill_names() {
        let names = get_skill_names();
        assert!(!names.is_empty());
        for name in &names {
            assert!(name.starts_with("ripwire-"));
        }
        assert!(names.contains(&"ripwire-orient"));
        assert!(names.contains(&"ripwire-change-check"));
    }

    #[test]
    fn test_generate_manifest_content() {
        let skills = ["ripwire-orient", "ripwire-change-check"];
        let manifest = generate_manifest_content(&skills);
        assert!(manifest.starts_with("version=1\n"));
        assert!(manifest.contains("skill=ripwire-orient\n"));
        assert!(manifest.contains("skill=ripwire-change-check\n"));
    }

    #[test]
    fn test_resolve_skills_dir() {
        let custom = resolve_skills_dir(Some("/custom/skills/dir"));
        assert_eq!(custom, PathBuf::from("/custom/skills/dir"));

        let agents_dir = resolve_skills_dir(Some("/some/path/.agents"));
        assert_eq!(agents_dir, PathBuf::from("/some/path/.agents/skills"));

        let default_dir = resolve_skills_dir(None);
        assert!(default_dir.ends_with(".agents/skills"));
    }

    #[test]
    fn test_expand_tilde() {
        assert_eq!(expand_tilde("/regular/path"), "/regular/path");
        let expanded = expand_tilde("~/test");
        assert!(!expanded.starts_with('~'));
    }

    #[test]
    fn test_embedded_skills_integrity() {
        assert!(!EMBEDDED_SKILLS.is_empty());
        for (dest, content) in EMBEDDED_SKILLS {
            assert!(dest.ends_with(".md"));
            assert!(!content.is_empty());
            assert!(content.contains("---") || content.contains('#'));
        }
    }
}
