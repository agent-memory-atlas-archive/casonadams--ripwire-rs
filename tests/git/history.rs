use ripwire_rs::git::history::{
    CommitInfo, SinceScope, civil_from_days, compute_file_churn, days_from_civil,
    filter_commits_by_scope, months_before_epoch, parse_since_scope, walk_repository_history,
    walk_repository_history_scoped,
};

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
    let epoch = 1775000000;
    let commits = vec![
        CommitInfo {
            id: "c3".to_string(),
            timestamp: epoch,
            author: "alice@example.com".to_string(),
            changed_files: vec!["a.rs".to_string(), "b.rs".to_string()],
        },
        CommitInfo {
            id: "c2".to_string(),
            timestamp: epoch - 90 * 86400,
            author: "alice@example.com".to_string(),
            changed_files: vec!["a.rs".to_string()],
        },
        CommitInfo {
            id: "c1".to_string(),
            timestamp: epoch - 180 * 86400,
            author: "bob@example.com".to_string(),
            changed_files: vec!["b.rs".to_string()],
        },
        CommitInfo {
            id: "bomb".to_string(),
            timestamp: epoch - 100,
            author: "dev@example.com".to_string(),
            changed_files: (0..150).map(|i| format!("file_{i}.rs")).collect(),
        },
    ];
    let filtered = filter_commits_by_scope(&commits, &SinceScope::SinceDate(epoch - 90 * 86400));
    assert_eq!(filtered.len(), 3);
    assert_eq!(filtered[0].id, "c3");
    assert_eq!(filtered[1].id, "c2");

    let rev_filtered =
        filter_commits_by_scope(&commits, &SinceScope::SinceRevision("c2".to_string()));
    assert_eq!(rev_filtered.len(), 1);
    assert_eq!(rev_filtered[0].id, "c3");

    let churn = compute_file_churn(commits.iter(), epoch, 100);
    assert_eq!(churn.len(), 2);
    assert_eq!(churn["a.rs"].commits, 2);
    assert_eq!(churn["b.rs"].commits, 2);
    assert_eq!(churn["a.rs"].last_commit_epoch, epoch);
    assert_eq!(churn["b.rs"].last_commit_epoch, epoch);
    assert!((churn["a.rs"].decayed_weight - 1.5).abs() < 0.05);
    assert!(!churn.contains_key("file_0.rs"));
}

#[test]
fn test_walk_repository_history_nonexistent() {
    let res = walk_repository_history("/nonexistent_ripwire_path_test");
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(format!("{err}").contains("failed to open repository"));
}

#[test]
fn test_history_walk_is_newest_first_non_merge_with_exact_cutoff() {
    let repo = gix::discover(".").unwrap();
    let recent_tip = repo.head_commit().unwrap().time().unwrap().seconds - 30 * 86_400;
    let commits = walk_repository_history_scoped(".", Some(recent_tip)).unwrap();
    assert!(commits.windows(2).all(|w| w[0].timestamp >= w[1].timestamp));
    assert!(commits.iter().all(|c| {
        let id = gix::ObjectId::from_hex(c.id.as_bytes()).unwrap();
        repo.find_commit(id).unwrap().parent_ids().count() <= 1
    }));
    if commits.is_empty() {
        return;
    }
    let cutoff = commits[commits.len() / 2].timestamp;
    let newer = walk_repository_history_scoped(".", Some(cutoff)).unwrap();
    let expected: Vec<&str> = commits
        .iter()
        .filter(|c| c.timestamp >= cutoff)
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(
        newer.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        expected
    );
}
