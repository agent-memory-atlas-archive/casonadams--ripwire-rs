use crate::domain::{IngestResult, Interner};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;

/// Execute `mentions` tool: find documentation naming a symbol in backticks.
pub fn call_mentions(ingest: &IngestResult, interner: &Interner, symbol_name: &str) -> Value {
    let mut matching_docs = Vec::new();
    let backtick_target = format!("`{symbol_name}`");

    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        if (path.ends_with(".md") || path.contains("doc"))
            && let Ok(content) = fs::read_to_string(path)
        {
            let occurrences = content.matches(&backtick_target).count();
            if occurrences > 0 {
                matching_docs.push(json!({
                    "file": path,
                    "mentions": occurrences
                }));
            }
        }
    }

    json!({
        "symbol": symbol_name,
        "docs": matching_docs,
        "total_mentions": matching_docs.iter().map(|d| d["mentions"].as_u64().unwrap_or(0)).sum::<u64>()
    })
}

/// Execute `doc_drift` tool: verify checkable doc references.
fn check_word_drift(
    word: &str,
    doc_path: &str,
    doc_line: usize,
    file_map: &HashMap<&str, u32>,
) -> Option<Value> {
    let (target_file, line_str) = word.split_once(':')?;
    let target_line = line_str
        .trim_matches(|c: char| !c.is_ascii_digit())
        .parse::<u32>()
        .ok()?;
    let &total_lines = file_map.get(target_file)?;
    if target_line > total_lines {
        Some(json!({
            "doc": doc_path,
            "doc_line": doc_line,
            "target_file": target_file,
            "claimed_line": target_line,
            "actual_lines": total_lines,
            "why": "line-out-of-bounds"
        }))
    } else {
        None
    }
}

/// Execute `doc_drift` tool: verify checkable doc references.
pub fn call_doc_drift(ingest: &IngestResult, interner: &Interner) -> Value {
    let mut drift_items = Vec::new();

    let file_map: HashMap<&str, u32> = ingest
        .files
        .iter()
        .map(|f| (interner.resolve(&f.path), f.line_count))
        .collect();

    for file in &ingest.files {
        let doc_path = interner.resolve(&file.path);
        if !doc_path.ends_with(".md") {
            continue;
        }

        let Ok(text) = fs::read_to_string(doc_path) else {
            continue;
        };
        for (line_idx, line) in text.lines().enumerate() {
            for word in line.split_whitespace() {
                if let Some(item) = check_word_drift(word, doc_path, line_idx + 1, &file_map) {
                    drift_items.push(item);
                }
            }
        }
    }

    json!({
        "drift_count": drift_items.len(),
        "drift": drift_items
    })
}

/// Execute `flags` tool: detect build flags and runtime configuration gates.
pub fn call_flags(ingest: &IngestResult, interner: &Interner) -> Value {
    let mut flags = Vec::new();

    for file in &ingest.files {
        let path = interner.resolve(&file.path);
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };

        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("#ifndef ") || trimmed.starts_with("#ifdef ") {
                let flag = trimmed.split_whitespace().nth(1).unwrap_or("");
                flags.push(json!({
                    "name": flag,
                    "flag": flag,
                    "kind": "compile",
                    "file": path,
                    "path": path,
                    "line": idx + 1
                }));
            } else if trimmed.contains("env::var(") || trimmed.contains("getenv(") {
                flags.push(json!({
                    "name": trimmed,
                    "flag": trimmed,
                    "kind": "env",
                    "file": path,
                    "path": path,
                    "line": idx + 1
                }));
            }
        }
    }

    json!({
        "flags_count": flags.len(),
        "gates": flags.clone(),
        "flags": flags
    })
}

/// Execute `owners` tool: git commit author ownership per file.
fn compute_file_author_weights(
    commits: &[crate::git::history::CommitInfo],
    path_filter: Option<&str>,
) -> HashMap<String, HashMap<String, f64>> {
    let mut file_authors: HashMap<String, HashMap<String, f64>> = HashMap::new();
    let head_epoch = commits.first().map_or(0, |c| c.timestamp);

    for c in commits {
        let age_sec = (head_epoch - c.timestamp).max(0);
        let age_days = (age_sec as f64) / 86400.0;
        let weight = 0.5f64.powf(age_days / 180.0);
        let author = if c.author.is_empty() {
            "unknown".to_string()
        } else {
            c.author.clone()
        };

        for f in &c.changed_files {
            if path_filter.is_none() || path_filter.is_some_and(|pf| f.contains(pf)) {
                let entry = file_authors.entry(f.clone()).or_default();
                *entry.entry(author.clone()).or_insert(0.0) += weight;
            }
        }
    }
    file_authors
}

fn score_file_ownership(f: String, author_weights: HashMap<String, f64>) -> Value {
    let authors_count = author_weights.len();
    let total_weight: f64 = author_weights.values().sum();
    let (top_author, top_weight) = author_weights
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or_default();
    let share = if total_weight > 0.0 {
        top_weight / total_weight
    } else {
        1.0
    };
    let bf = if share >= 0.80 { 1 } else { 0 };

    json!({
        "file": f.clone(),
        "path": f,
        "authors": authors_count,
        "bf": bf,
        "top": top_author.clone(),
        "top_author": top_author,
        "share": (share * 100.0).round() / 100.0,
        "top_share": (share * 100.0).round() / 100.0,
    })
}

/// Execute `owners` tool: git commit author ownership per file.
pub fn call_owners(
    commits: Option<&[crate::git::history::CommitInfo]>,
    path_filter: Option<&str>,
) -> Value {
    let Some(commits) = commits else {
        return json!({ "error": "git commit history unavailable" });
    };

    let file_authors = compute_file_author_weights(commits, path_filter);
    let mut file_results: Vec<Value> = file_authors
        .into_iter()
        .map(|(f, map)| score_file_ownership(f, map))
        .collect();

    file_results.sort_by(|a, b| {
        let path_a = a["path"].as_str().unwrap_or("");
        let path_b = b["path"].as_str().unwrap_or("");
        path_a.cmp(path_b)
    });

    json!({
        "files_analyzed": file_results.len(),
        "files": file_results.clone(),
        "ownership": file_results
    })
}

/// Execute `whereis` tool: identify where a symbol lives (definitions and references).
pub fn call_whereis(ingest: &IngestResult, interner: &Interner, symbol_name: &str) -> Value {
    let mut hits = Vec::new();

    for sym in &ingest.symbols {
        let n = interner.resolve(&sym.name);
        if n == symbol_name {
            let file_path = ingest
                .files
                .get(sym.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            hits.push(json!({
                "symbol": n,
                "kind": "def",
                "file": file_path,
                "path": file_path,
                "line": sym.line,
                "text": format!("{} {}", sym.kind.tag(), n),
                "on_head": true
            }));
        }
    }

    for r in &ingest.references {
        let n = interner.resolve(&r.callee_name);
        if n == symbol_name {
            let file_path = ingest
                .files
                .get(r.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            hits.push(json!({
                "symbol": n,
                "kind": "ref",
                "file": file_path,
                "path": file_path,
                "line": r.line,
                "text": n,
                "on_head": true
            }));
        }
    }

    json!({
        "symbol": symbol_name,
        "count": hits.len(),
        "hits": hits.clone(),
        "locations": hits
    })
}

fn find_stray_branches() -> Vec<Value> {
    let Ok(output) = std::process::Command::new("git")
        .args(["for-each-ref", "--format=%(refname:short)", "refs/heads/"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut branches = Vec::new();
    for line in text.lines() {
        let branch = line.trim();
        if branch.is_empty() || branch == "main" || branch == "master" || branch == "HEAD" {
            continue;
        }
        let Ok(count_out) = std::process::Command::new("git")
            .args(["rev-list", "--count", &format!("HEAD..{branch}")])
            .output()
        else {
            continue;
        };
        if count_out.status.success() {
            let count_str = String::from_utf8_lossy(&count_out.stdout)
                .trim()
                .to_string();
            let unmerged = count_str.parse::<u64>().unwrap_or(0);
            if unmerged > 0 {
                branches.push(json!({
                    "branch": branch,
                    "unmerged_commits": unmerged,
                }));
            }
        }
    }
    branches
}

/// Execute `stray_content` tool.
pub fn call_stray_content(commits: Option<&[crate::git::history::CommitInfo]>) -> Value {
    let commit_count = commits.map(|c| c.len()).unwrap_or(0);
    let branches = find_stray_branches();
    let stray_count = branches.len();
    json!({
        "stray_count": stray_count,
        "commits_inspected": commit_count,
        "branches": branches,
        "status": if stray_count == 0 { "clean" } else { "diverged" }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mentions_empty() {
        let ingest = IngestResult::new();
        let interner = Interner::new();
        let res = call_mentions(&ingest, &interner, "nonexistent");
        assert_eq!(res["total_mentions"], 0);
    }

    #[test]
    fn test_owners_attribution() {
        let commits = vec![
            crate::git::history::CommitInfo {
                id: "c1".to_string(),
                timestamp: 1000,
                author: "alice@example.com".to_string(),
                changed_files: vec!["src/lib.rs".to_string()],
            },
            crate::git::history::CommitInfo {
                id: "c2".to_string(),
                timestamp: 2000,
                author: "bob@example.com".to_string(),
                changed_files: vec!["src/lib.rs".to_string()],
            },
        ];
        let res = call_owners(Some(&commits), None);
        assert_eq!(res["files_analyzed"], 1);
        let files = res["files"].as_array().unwrap();
        assert_eq!(files[0]["authors"], 2);
    }

    #[test]
    fn test_whereis_and_stray() {
        let mut ingest = IngestResult::new();
        let interner = Interner::new();
        let f0 = crate::domain::FileInfo {
            id: 0,
            path: interner.intern("src/main.rs"),
            lang: crate::domain::Lang::Rust,
            line_count: 10,
            byte_size: 100,
        };
        let sym = crate::domain::Symbol::new(
            0,
            interner.intern("main"),
            interner.intern("c_main"),
            crate::domain::SymKind::Function,
            crate::domain::Lang::Rust,
            0,
            1,
        );
        ingest.files.push(f0);
        ingest.symbols.push(sym);

        let whereis_res = call_whereis(&ingest, &interner, "main");
        assert_eq!(whereis_res["count"], 1);

        let stray_res = call_stray_content(None);
        assert!(stray_res.get("stray_count").is_some());
    }
}
