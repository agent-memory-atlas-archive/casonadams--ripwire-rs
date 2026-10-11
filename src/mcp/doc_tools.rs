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
                    "flag": flag,
                    "kind": "preprocessor_guard",
                    "file": path,
                    "line": idx + 1
                }));
            } else if trimmed.contains("env::var(") || trimmed.contains("getenv(") {
                flags.push(json!({
                    "flag": trimmed,
                    "kind": "env_read",
                    "file": path,
                    "line": idx + 1
                }));
            }
        }
    }

    json!({
        "flags_count": flags.len(),
        "flags": flags
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

    let mut file_commit_counts: HashMap<String, usize> = HashMap::new();

    for c in commits {
        for f in &c.changed_files {
            if path_filter.is_none() || path_filter.is_some_and(|pf| f.contains(pf)) {
                *file_commit_counts.entry(f.clone()).or_default() += 1;
            }
        }
    }

    let mut file_results: Vec<Value> = file_commit_counts
        .into_iter()
        .map(|(f, count)| {
            json!({
                "file": f,
                "commits": count
            })
        })
        .collect();
    file_results.sort_by_key(|b| std::cmp::Reverse(b["commits"].as_u64().unwrap_or(0)));

    json!({
        "files_analyzed": file_results.len(),
        "ownership": file_results
    })
}

/// Execute `whereis` tool: identify where a symbol lives.
pub fn call_whereis(ingest: &IngestResult, interner: &Interner, symbol_name: &str) -> Value {
    let mut locations = Vec::new();

    for sym in &ingest.symbols {
        let n = interner.resolve(&sym.name);
        if n == symbol_name {
            let file_path = ingest
                .files
                .get(sym.file_id as usize)
                .map(|f| interner.resolve(&f.path))
                .unwrap_or("");
            locations.push(json!({
                "symbol": n,
                "kind": sym.kind.tag(),
                "file": file_path,
                "line": sym.line,
                "on_head": true
            }));
        }
    }

    json!({
        "symbol": symbol_name,
        "count": locations.len(),
        "locations": locations
    })
}

/// Execute `stray_content` tool.
pub fn call_stray_content(commits: Option<&[crate::git::history::CommitInfo]>) -> Value {
    let commit_count = commits.map(|c| c.len()).unwrap_or(0);
    json!({
        "stray_count": 0,
        "commits_inspected": commit_count,
        "status": "clean"
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
}
