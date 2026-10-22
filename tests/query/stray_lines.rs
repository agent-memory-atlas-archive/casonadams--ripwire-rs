use ripwire_rs::query::branch_tools::{is_stray_candidate, stray_content_value};
use ripwire_rs::query::stray_lines::{BranchVerdict, branch_verdict, classify_file};

const BASE: &str = "fn a() {\n    old_one();\n    old_two();\n}\n";

#[test]
fn pure_addition_absent_from_head_is_all_stray() {
    let branch = "fn a() {}\nfn fresh() {\n    new_work();\n}\n";
    let stat = classify_file(BASE, branch, BASE, false);
    assert_eq!(stat.authored, 3);
    assert_eq!(stat.del, 3);
    assert_eq!(stat.stray, 3);
    assert!(!stat.superseded, "HEAD never touched the file");
    assert_eq!(stat.verdict(), "unmerged");
}

#[test]
fn head_reimplementing_removed_base_lines_supersedes_the_branch() {
    let branch = "fn a() {\n    rewritten_once();\n}\n";
    let head = "fn a() {\n    rewritten_twice();\n}\n";
    let stat = classify_file(BASE, branch, head, true);
    assert_eq!(stat.authored, 1);
    assert_eq!(stat.stray, 1, "the branch's line is not on HEAD");
    assert_eq!(stat.del, 2);
    assert_eq!(
        stat.redone, 2,
        "HEAD removed both base lines the branch removed"
    );
    assert!(stat.superseded);
    assert_eq!(stat.verdict(), "superseded");
}

#[test]
fn head_holding_the_branch_lines_leaves_no_stray_to_supersede() {
    let branch = "fn a() {\n    replacement();\n}\n";
    let head = "fn a() {\n    replacement();\n}\n";
    let stat = classify_file(BASE, branch, head, true);
    assert_eq!(stat.del, 2);
    assert_eq!(stat.redone, 2);
    assert_eq!(stat.stray, 0, "HEAD already holds the branch's line");
    assert!(
        !stat.superseded,
        "no stray lines means nothing to supersede"
    );
}

#[test]
fn head_reworking_only_part_of_the_removed_base_is_unmerged() {
    let branch = "fn a() {\n    fresh_one();\n    fresh_two();\n}\n";
    let head = "fn a() {\n    old_one();\n    different();\n}\n";
    let stat = classify_file(BASE, branch, head, true);
    assert_eq!(stat.del, 2);
    assert_eq!(stat.redone, 1, "only old_two was removed by HEAD");
    assert_eq!(stat.stray, 2);
    assert!(
        !stat.superseded,
        "1 of 2 removed lines is below the supersession share"
    );
}

#[test]
fn head_untouched_file_is_never_superseded() {
    let branch = "fn a() {\n    fresh_one();\n}\n";
    let stat = classify_file(BASE, branch, BASE, false);
    assert_eq!(stat.redone, 0);
    assert!(!stat.superseded);
}

#[test]
fn whitespace_and_blank_lines_do_not_count_as_authored_work() {
    let branch = "fn a() {\n\n        old_one();   \n    old_two();\n}\n";
    let stat = classify_file(BASE, branch, BASE, true);
    assert_eq!(stat.authored, 0);
    assert_eq!(stat.stray, 0);
    assert!((stat.sim - 1.0).abs() < f64::EPSILON);
}

#[test]
fn similarity_is_the_share_of_branch_lines_head_already_has() {
    let branch = "alpha\nbeta\ngamma\ndelta\n";
    let head = "alpha\nbeta\n";
    let stat = classify_file("", branch, head, true);
    assert_eq!(stat.authored, 4);
    assert_eq!(stat.stray, 2);
    assert!((stat.sim - 0.5).abs() < f64::EPSILON);
    assert!(
        !stat.superseded,
        "pure addition with half the lines missing stays unmerged"
    );
}

#[test]
fn empty_branch_text_has_no_stray_or_similarity() {
    let stat = classify_file(BASE, "", BASE, true);
    assert_eq!(stat.authored, 0);
    assert_eq!(stat.stray, 0);
    assert_eq!(stat.sim, 0.0);
}

#[test]
fn branch_verdict_is_merged_without_stray_lines() {
    assert_eq!(branch_verdict(0, 0), BranchVerdict::Merged);
    assert_eq!(BranchVerdict::Merged.as_str(), "merged");
}

#[test]
fn branch_verdict_needs_a_majority_of_stray_lines_in_superseded_files() {
    assert_eq!(branch_verdict(10, 6), BranchVerdict::Superseded);
    assert_eq!(branch_verdict(10, 5), BranchVerdict::Unmerged);
    assert_eq!(branch_verdict(10, 0), BranchVerdict::Unmerged);
}

#[test]
fn stray_candidates_exclude_head_checked_out_and_head_commit_refs() {
    assert!(is_stray_candidate("feature", "main", Some(2), Some(1)));
    assert!(!is_stray_candidate("main", "main", Some(2), Some(1)));
    assert!(!is_stray_candidate("HEAD", "main", Some(2), Some(1)));
    assert!(!is_stray_candidate("", "main", Some(2), Some(1)));
    assert!(!is_stray_candidate("alias", "main", Some(1), Some(1)));
}

#[test]
fn stray_content_tallies_verdicts_and_lists_only_stray_refs() {
    let reports = vec![
        Some(serde_json::json!({"name": "a", "verdict": "unmerged", "stray": 4, "superseded": 0})),
        Some(
            serde_json::json!({"name": "b", "verdict": "superseded", "stray": 2, "superseded": 2}),
        ),
        Some(serde_json::json!({"name": "c", "verdict": "merged", "stray": 0, "superseded": 0})),
        None,
    ];
    let out = stray_content_value("at1".into(), "head1".into(), "main".into(), 7, reports);
    assert_eq!(out["refs"], 4);
    assert_eq!(out["unmerged"], 1);
    assert_eq!(out["superseded"], 1);
    assert_eq!(out["merged"], 1);
    assert_eq!(out["unknown"], 1);
    assert_eq!(out["blobs"], 7);
    assert_eq!(out["head"], "head1");
    assert_eq!(out["at"], "at1");
    let names: Vec<&str> = out["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["a", "b"]);
}

#[test]
fn stray_content_with_no_refs_reports_zero_counts() {
    let out = stray_content_value(String::new(), String::new(), "HEAD".into(), 0, Vec::new());
    assert_eq!(out["refs"], 0);
    assert_eq!(out["branches"].as_array().unwrap().len(), 0);
}
