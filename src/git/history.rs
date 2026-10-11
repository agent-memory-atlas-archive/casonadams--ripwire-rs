use std::collections::HashMap;
use std::fmt;
use std::path::Path;

pub const CHURN_DECAY_HALF_LIFE_DAYS: f64 = 90.0;
pub const MERGE_BOMB_MAX_FILES: usize = 100;
pub const DEFAULT_CHURN_MONTHS: u32 = 12;
pub const DEFAULT_COCHANGE_MONTHS: u32 = 18;

/// A parsed git commit with its timestamp and changed files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub id: String,
    pub timestamp: i64,
    pub changed_files: Vec<String>,
}

/// The time or revision boundary for git mining.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinceScope {
    AllHistory,
    SinceDate(i64),
    SinceRevision(String),
}

/// Churn metrics computed for a single file.
#[derive(Debug, Clone, PartialEq)]
pub struct FileChurn {
    pub file_path: String,
    pub commits: u32,
    pub decayed_weight: f64,
    pub last_commit_epoch: i64,
}

#[derive(Debug)]
pub enum GitHistoryError {
    Open(String),
    Head(String),
    Walk(String),
    TreeDiff(String),
}

impl fmt::Display for GitHistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(msg) => write!(f, "failed to open repository: {msg}"),
            Self::Head(msg) => write!(f, "failed to resolve HEAD: {msg}"),
            Self::Walk(msg) => write!(f, "failed walking revisions: {msg}"),
            Self::TreeDiff(msg) => write!(f, "failed diffing trees: {msg}"),
        }
    }
}

impl std::error::Error for GitHistoryError {}

/// Convert a civil calendar date (Y, M, D) to days since Unix epoch (1970-01-01).
#[must_use]
pub fn days_from_civil(mut y: i64, m: u32, d: u32) -> i64 {
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let m_adj = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * m_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + (doe as i64) - 719468
}

/// Convert days since Unix epoch to civil date (year, month, day).
#[must_use]
pub fn civil_from_days(mut z: i64) -> (i64, u32, u32) {
    z += 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = (yoe as i64) + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

/// Calculate the timestamp `months` calendar months before `epoch`.
#[must_use]
pub fn months_before_epoch(epoch: i64, months: u32) -> i64 {
    let days = if epoch >= 0 {
        epoch / 86400
    } else {
        -(-epoch + 86399) / 86400
    };
    let sec_of_day = epoch - days * 86400;
    let (y, m, d) = civil_from_days(days);
    let month_idx = y * 12 + (m as i64) - 1 - (months as i64);
    let ny = if month_idx >= 0 {
        month_idx / 12
    } else {
        -(-month_idx + 11) / 12
    };
    let nm = (month_idx - ny * 12 + 1) as u32;
    (days_from_civil(ny, nm, 1) + (d as i64) - 1) * 86400 + sec_of_day
}

fn parse_numeric_prefix(s: &str) -> Option<(u32, &str)> {
    let trimmed = s.trim_start();
    let num_end = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    if num_end == 0 {
        return None;
    }
    let n = trimmed[..num_end].parse::<u32>().ok()?;
    Some((n, trimmed[num_end..].trim()))
}

fn parse_relative_date(s: &str, anchor: i64) -> Option<i64> {
    let lower = s.to_ascii_lowercase();
    if lower == "today" {
        return Some(anchor);
    }
    if lower == "yesterday" {
        return Some(anchor - 86400);
    }
    let (n, rest) = parse_numeric_prefix(&lower)?;
    let unit = rest.strip_suffix("ago").unwrap_or(rest).trim();
    if unit.starts_with("month") {
        Some(months_before_epoch(anchor, n))
    } else if unit.starts_with("week") {
        Some(anchor - (n as i64) * 7 * 86400)
    } else if unit.starts_with("day") {
        Some(anchor - (n as i64) * 86400)
    } else if unit.starts_with("year") {
        Some(months_before_epoch(anchor, n * 12))
    } else {
        None
    }
}

fn parse_iso_date(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split(['-', 'T', 'Z', ':']).collect();
    if parts.len() < 3 {
        return None;
    }
    let y = parts[0].parse::<i64>().ok()?;
    let m = parts[1].parse::<u32>().ok()?;
    let d = parts[2].parse::<u32>().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let days = days_from_civil(y, m, d);
    let mut secs = days * 86400;
    if parts.len() >= 6 {
        let hour = parts[3].parse::<i64>().ok()?;
        let min = parts[4].parse::<i64>().ok()?;
        let sec = parts[5].parse::<i64>().ok()?;
        secs += hour * 3600 + min * 60 + sec;
    }
    Some(secs)
}

fn is_commit_sha(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Parse a `--since` string into a `SinceScope`.
#[must_use]
pub fn parse_since_scope(since_str: Option<&str>, anchor_epoch: i64) -> SinceScope {
    let s = match since_str {
        Some(v) if !v.trim().is_empty() => v.trim(),
        _ => return SinceScope::AllHistory,
    };
    if is_commit_sha(s) || s.starts_with("HEAD~") || s.starts_with('v') {
        return SinceScope::SinceRevision(s.to_string());
    }
    if let Some(epoch) = parse_relative_date(s, anchor_epoch) {
        return SinceScope::SinceDate(epoch);
    }
    if let Some(epoch) = parse_iso_date(s) {
        return SinceScope::SinceDate(epoch);
    }
    SinceScope::SinceRevision(s.to_string())
}

/// Filter commits according to the specified `SinceScope`.
#[must_use]
pub fn filter_commits_by_scope<'a>(
    commits: &'a [CommitInfo],
    scope: &SinceScope,
) -> Vec<&'a CommitInfo> {
    match scope {
        SinceScope::AllHistory => commits.iter().collect(),
        SinceScope::SinceDate(cutoff) => {
            commits.iter().filter(|c| c.timestamp >= *cutoff).collect()
        }
        SinceScope::SinceRevision(rev) => {
            let mut out = Vec::new();
            for c in commits {
                if c.id == *rev || c.id.starts_with(rev) {
                    break;
                }
                out.push(c);
            }
            out
        }
    }
}

/// Compute raw commit churn, time-decayed churn weight, and newest commit epoch per file.
#[must_use]
pub fn compute_file_churn<'a, I>(
    commits: I,
    head_epoch: i64,
    max_files_filter: usize,
) -> HashMap<String, FileChurn>
where
    I: IntoIterator<Item = &'a CommitInfo>,
{
    let mut churn_map: HashMap<String, FileChurn> = HashMap::new();
    for commit in commits {
        if commit.changed_files.len() > max_files_filter {
            continue;
        }
        let age_sec = (head_epoch - commit.timestamp).max(0);
        let age_days = (age_sec as f64) / 86400.0;
        let weight = 0.5f64.powf(age_days / CHURN_DECAY_HALF_LIFE_DAYS);

        for file in &commit.changed_files {
            let entry = churn_map.entry(file.clone()).or_insert_with(|| FileChurn {
                file_path: file.clone(),
                commits: 0,
                decayed_weight: 0.0,
                last_commit_epoch: 0,
            });
            entry.commits += 1;
            entry.decayed_weight += weight;
            if commit.timestamp > entry.last_commit_epoch {
                entry.last_commit_epoch = commit.timestamp;
            }
        }
    }
    churn_map
}

fn get_parent_or_empty_tree<'a>(
    repo: &'a gix::Repository,
    commit: &gix::Commit<'_>,
) -> Result<gix::Tree<'a>, GitHistoryError> {
    if let Some(parent_id) = commit.parent_ids().next() {
        let parent = repo
            .find_object(parent_id)
            .map_err(|e| GitHistoryError::TreeDiff(e.to_string()))?
            .into_commit();
        parent
            .tree()
            .map_err(|e| GitHistoryError::TreeDiff(e.to_string()))
    } else {
        Ok(repo.empty_tree())
    }
}

fn extract_commit_changed_files(
    repo: &gix::Repository,
    commit: &gix::Commit<'_>,
    tree: &gix::Tree<'_>,
) -> Result<Vec<String>, GitHistoryError> {
    let mut changed_files = Vec::new();
    let parent_tree = get_parent_or_empty_tree(repo, commit)?;
    let mut changes = parent_tree
        .changes()
        .map_err(|e| GitHistoryError::TreeDiff(e.to_string()))?;
    changes
        .for_each_to_obtain_tree(tree, |change| {
            changed_files.push(change.location().to_string());
            Ok::<_, std::convert::Infallible>(gix::object::tree::diff::Action::Continue)
        })
        .map_err(|e| GitHistoryError::TreeDiff(e.to_string()))?;
    Ok(changed_files)
}

fn parse_commit_record(
    repo: &gix::Repository,
    info: gix::revision::walk::Info,
) -> Result<CommitInfo, GitHistoryError> {
    let commit = info
        .object()
        .map_err(|e| GitHistoryError::Walk(e.to_string()))?;
    let time = commit
        .time()
        .map_err(|e| GitHistoryError::Walk(e.to_string()))?;
    let tree = commit
        .tree()
        .map_err(|e| GitHistoryError::Walk(e.to_string()))?;
    let changed_files = extract_commit_changed_files(repo, &commit, &tree)?;
    Ok(CommitInfo {
        id: info.id.to_string(),
        timestamp: time.seconds,
        changed_files,
    })
}

fn collect_rev_walk<'a>(
    repo: &gix::Repository,
    rev_walk: impl IntoIterator<
        Item = Result<gix::revision::walk::Info<'a>, gix::revision::walk::iter::Error>,
    >,
) -> Result<Vec<CommitInfo>, GitHistoryError> {
    let mut commits = Vec::new();
    for info in rev_walk {
        let info = info.map_err(|e| GitHistoryError::Walk(e.to_string()))?;
        commits.push(parse_commit_record(repo, info)?);
    }
    Ok(commits)
}

/// Walk git commit history and extract changed files per commit using `gix`.
pub fn walk_repository_history(repo_path: &Path) -> Result<Vec<CommitInfo>, GitHistoryError> {
    let repo = gix::open(repo_path).map_err(|e| GitHistoryError::Open(e.to_string()))?;
    let head = repo
        .head_commit()
        .map_err(|e| GitHistoryError::Head(e.to_string()))?;
    let rev_walk = repo
        .rev_walk([head.id()])
        .all()
        .map_err(|e| GitHistoryError::Walk(e.to_string()))?;
    collect_rev_walk(&repo, rev_walk)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_civil_calendar_conversions() {
        let (y, m, d) = civil_from_days(0);
        assert_eq!((y, m, d), (1970, 1, 1));
        assert_eq!(days_from_civil(1970, 1, 1), 0);

        let d2026 = days_from_civil(2026, 10, 11);
        let (y2, m2, d2) = civil_from_days(d2026);
        assert_eq!((y2, m2, d2), (2026, 10, 11));

        let epoch = 1775000000;
        let past = months_before_epoch(epoch, 12);
        assert!(past < epoch);
        assert_eq!(epoch - past, 365 * 86400);
    }

    #[test]
    fn test_since_scope_parsing() {
        let anchor = 1775000000;
        assert_eq!(parse_since_scope(None, anchor), SinceScope::AllHistory);
        assert_eq!(parse_since_scope(Some(""), anchor), SinceScope::AllHistory);
        assert_eq!(
            parse_since_scope(Some("today"), anchor),
            SinceScope::SinceDate(anchor)
        );
        assert_eq!(
            parse_since_scope(Some("yesterday"), anchor),
            SinceScope::SinceDate(anchor - 86400)
        );
        assert_eq!(
            parse_since_scope(Some("2 weeks ago"), anchor),
            SinceScope::SinceDate(anchor - 14 * 86400)
        );
        assert_eq!(
            parse_since_scope(Some("10 days ago"), anchor),
            SinceScope::SinceDate(anchor - 10 * 86400)
        );

        let sha = "e0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9";
        assert_eq!(
            parse_since_scope(Some(sha), anchor),
            SinceScope::SinceRevision(sha.to_string())
        );

        let iso = "2026-01-01";
        match parse_since_scope(Some(iso), anchor) {
            SinceScope::SinceDate(d) => assert!(d > 0),
            _ => panic!("expected SinceDate"),
        }
    }

    #[test]
    fn test_commit_filtering_and_churn() {
        let commits = vec![
            CommitInfo {
                id: "c3".to_string(),
                timestamp: 300,
                changed_files: vec!["a.rs".to_string(), "b.rs".to_string()],
            },
            CommitInfo {
                id: "c2".to_string(),
                timestamp: 200,
                changed_files: vec!["a.rs".to_string()],
            },
            CommitInfo {
                id: "c1".to_string(),
                timestamp: 100,
                changed_files: vec!["b.rs".to_string()],
            },
        ];

        let filtered = filter_commits_by_scope(&commits, &SinceScope::SinceDate(200));
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, "c3");
        assert_eq!(filtered[1].id, "c2");

        let rev_filtered =
            filter_commits_by_scope(&commits, &SinceScope::SinceRevision("c2".to_string()));
        assert_eq!(rev_filtered.len(), 1);
        assert_eq!(rev_filtered[0].id, "c3");

        let churn = compute_file_churn(commits.iter(), 300, 10);
        assert_eq!(churn.len(), 2);
        assert_eq!(churn["a.rs"].commits, 2);
        assert_eq!(churn["b.rs"].commits, 2);
        assert_eq!(churn["a.rs"].last_commit_epoch, 300);
        assert_eq!(churn["b.rs"].last_commit_epoch, 300);
        assert!(churn["a.rs"].decayed_weight > 1.99);
    }

    #[test]
    fn test_walk_repository_history_nonexistent() {
        let res = walk_repository_history(Path::new("/nonexistent_ripwire_path_test"));
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(format!("{err}").contains("failed to open repository"));
    }
}
