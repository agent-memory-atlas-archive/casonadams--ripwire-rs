//! Line-granular supersession for `--stray-content`.
//!
//! A branch's authored lines are the multiset difference between its file and the merge base.
//! A line is stray when HEAD lacks it. A file is superseded when HEAD re-implemented the work:
//! HEAD removed the base lines the branch removed, or the branch only added lines HEAD already has.
//! Lines are compared trimmed; blank lines carry no work and are ignored.
use std::collections::BTreeMap;

/// Trimmed non-blank line counts.
type Lines<'a> = BTreeMap<&'a str, usize>;

/// Stray evidence for one file of one branch.
#[derive(Debug, Clone, PartialEq)]
pub struct FileStray {
    pub stray: usize,
    pub authored: usize,
    pub del: usize,
    pub redone: usize,
    pub sim: f64,
    pub head_touched: bool,
    pub superseded: bool,
}

impl FileStray {
    /// `superseded` when HEAD re-implemented the work, otherwise `unmerged`.
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        if self.superseded {
            "superseded"
        } else {
            "unmerged"
        }
    }
}

/// Branch-level relation to HEAD, derived from its per-file stray lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchVerdict {
    Merged,
    Superseded,
    Unmerged,
}

impl BranchVerdict {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Merged => "merged",
            Self::Superseded => "superseded",
            Self::Unmerged => "unmerged",
        }
    }
}

/// Branch is merged when no stray lines remain, superseded when most stray lines sit in
/// superseded files, and unmerged otherwise.
#[must_use]
pub fn branch_verdict(stray: usize, superseded: usize) -> BranchVerdict {
    if stray == 0 {
        BranchVerdict::Merged
    } else if superseded * 2 > stray {
        BranchVerdict::Superseded
    } else {
        BranchVerdict::Unmerged
    }
}

fn line_counts(text: &str) -> Lines<'_> {
    let mut counts = Lines::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        *counts.entry(line).or_insert(0) += 1;
    }
    counts
}

/// Multiset `a - b`, keeping only positive counts.
fn excess<'a>(a: &Lines<'a>, b: &Lines<'_>) -> Lines<'a> {
    a.iter()
        .filter_map(|(line, &n)| {
            let left = n.saturating_sub(b.get(line).copied().unwrap_or(0));
            (left > 0).then_some((*line, left))
        })
        .collect()
}

fn total(counts: &Lines<'_>) -> usize {
    counts.values().sum()
}

/// Classify one file given its text at the merge base, on the branch, and at HEAD.
/// `head_touched` is true when HEAD changed this path since the merge base.
#[must_use]
pub fn classify_file(base: &str, branch: &str, head: &str, head_touched: bool) -> FileStray {
    let (base_lines, branch_lines, head_lines) =
        (line_counts(base), line_counts(branch), line_counts(head));
    let authored_lines = excess(&branch_lines, &base_lines);
    let removed_lines = excess(&base_lines, &branch_lines);
    let removed_by_head = excess(&base_lines, &head_lines);
    let authored = total(&authored_lines);
    let del = total(&removed_lines);
    let stray = total(&excess(&authored_lines, &head_lines));
    let redone: usize = removed_lines
        .iter()
        .map(|(line, &n)| n.min(removed_by_head.get(line).copied().unwrap_or(0)))
        .sum();
    let kept: usize = branch_lines
        .iter()
        .map(|(line, &n)| n.min(head_lines.get(line).copied().unwrap_or(0)))
        .sum();
    let branch_total = total(&branch_lines);
    let sim = if branch_total == 0 {
        0.0
    } else {
        kept as f64 / branch_total as f64
    };
    let supersedes = if del > 0 {
        redone * 5 >= del * 4
    } else {
        stray * 10 <= authored
    };
    FileStray {
        stray,
        authored,
        del,
        redone,
        sim,
        head_touched,
        superseded: head_touched && stray > 0 && supersedes,
    }
}
