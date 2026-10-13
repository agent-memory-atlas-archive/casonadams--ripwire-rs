use ripwire_rs::cli::execute_cochange_with;
use ripwire_rs::domain::{
    FileInfo, IngestResult, Interner, Lang, RefRole, Reference, SymKind, Symbol,
};
use ripwire_rs::git::cochange::{
    CochangeOptions, StaticCouplingMap, check_pair_dependency_capability, mine_cochange_for_file,
    mine_repo_cochange_pairs,
};
use ripwire_rs::git::history::{
    CommitInfo, SinceScope, civil_from_days, compute_file_churn, days_from_civil,
    filter_commits_by_scope, months_before_epoch, parse_since_scope,
};
use ripwire_rs::graph::Graph;
use ripwire_rs::mcp::{McpState, dispatch_json_rpc};
use serde_json::Value;
use std::io::Cursor;

fn create_test_ingest(interner: &Interner) -> IngestResult {
    let mut ingest = IngestResult::new();

    // File 0: src/caller.rs (Rust)
    let p0 = interner.intern("src/caller.rs");
    ingest.files.push(FileInfo {
        id: 0,
        path: p0,
        lang: Lang::Rust,
        line_count: 50,
        byte_size: 1000,
    });

    // File 1: src/callee.rs (Rust)
    let p1 = interner.intern("src/callee.rs");
    ingest.files.push(FileInfo {
        id: 1,
        path: p1,
        lang: Lang::Rust,
        line_count: 40,
        byte_size: 800,
    });

    // File 2: src/unrelated.rs (Rust)
    let p2 = interner.intern("src/unrelated.rs");
    ingest.files.push(FileInfo {
        id: 2,
        path: p2,
        lang: Lang::Rust,
        line_count: 60,
        byte_size: 1200,
    });

    // File 3: docs/arch.md (Markdown)
    let p3 = interner.intern("docs/arch.md");
    ingest.files.push(FileInfo {
        id: 3,
        path: p3,
        lang: Lang::Markdown,
        line_count: 100,
        byte_size: 2000,
    });
    // Symbol 0: foo() in src/caller.rs
    let s0_name = interner.intern("foo");
    ingest.symbols.push(Symbol::new(
        0,
        s0_name,
        s0_name,
        SymKind::Function,
        Lang::Rust,
        0,
        5,
    ));

    // Symbol 1: bar() in src/callee.rs
    let s1_name = interner.intern("bar");
    ingest.symbols.push(Symbol::new(
        1,
        s1_name,
        s1_name,
        SymKind::Function,
        Lang::Rust,
        1,
        5,
    ));

    // Reference from foo() in src/caller.rs -> bar() in src/callee.rs
    ingest
        .references
        .push(Reference::new(0, s1_name, 0, 8, Lang::Rust, RefRole::Call));

    ingest
}

#[test]
fn test_git_calendar_and_since_filtering() {
    let epoch = 1775000000;
    let days = days_from_civil(2026, 10, 11);
    let (y, m, d) = civil_from_days(days);
    assert_eq!((y, m, d), (2026, 10, 11));

    let past = months_before_epoch(epoch, 6);
    assert!(past < epoch);

    let scope_all = parse_since_scope(None, epoch);
    assert_eq!(scope_all, SinceScope::AllHistory);

    let scope_date = parse_since_scope(Some("2 weeks ago"), epoch);
    match scope_date {
        SinceScope::SinceDate(ts) => assert_eq!(ts, epoch - 14 * 86400),
        _ => panic!("expected SinceDate"),
    }

    let commits = vec![
        CommitInfo {
            id: "sha3".to_string(),
            timestamp: epoch,
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/caller.rs".to_string()],
        },
        CommitInfo {
            id: "sha2".to_string(),
            timestamp: epoch - 7 * 86400,
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/caller.rs".to_string(), "src/callee.rs".to_string()],
        },
        CommitInfo {
            id: "sha1".to_string(),
            timestamp: epoch - 30 * 86400,
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/unrelated.rs".to_string()],
        },
    ];

    let filtered = filter_commits_by_scope(&commits, &scope_date);
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].id, "sha3");
    assert_eq!(filtered[1].id, "sha2");
}

#[test]
fn test_git_file_churn_and_decay() {
    let epoch = 1775000000;
    let commits = vec![
        CommitInfo {
            id: "c3".to_string(),
            timestamp: epoch,
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/caller.rs".to_string(), "src/callee.rs".to_string()],
        },
        CommitInfo {
            id: "c2".to_string(),
            timestamp: epoch - 90 * 86400, // 90 days ago: weight should be ~0.5
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/caller.rs".to_string()],
        },
        CommitInfo {
            id: "bomb".to_string(),
            timestamp: epoch - 100,
            author: "dev@example.com".to_string(),
            changed_files: (0..150).map(|i| format!("file_{i}.rs")).collect(),
        },
    ];

    let churn = compute_file_churn(&commits, epoch, 100);
    assert_eq!(churn["src/caller.rs"].commits, 2);
    assert_eq!(churn["src/caller.rs"].last_commit_epoch, epoch);

    // Commit at epoch contributes 1.0, commit at 90 days ago contributes 0.5
    let w = churn["src/caller.rs"].decayed_weight;
    assert!((w - 1.5).abs() < 0.05, "expected ~1.5 weight, got {w}");

    // The merge bomb commit (> 100 files) was skipped
    assert!(!churn.contains_key("file_0.rs"));
}

#[test]
fn test_git_cochange_surprising_and_recurrence() {
    let interner = Interner::new();
    let ingest = create_test_ingest(&interner);
    let graph = Graph::build(&ingest, &interner);
    let coupling = StaticCouplingMap::build(&ingest, &interner, Some(&graph));

    // src/caller.rs calls src/callee.rs -> statically coupled!
    assert!(coupling.is_coupled("src/caller.rs", "src/callee.rs"));
    assert!(coupling.is_coupled("src/callee.rs", "src/caller.rs"));

    // src/caller.rs does NOT call src/unrelated.rs -> not coupled
    assert!(!coupling.is_coupled("src/caller.rs", "src/unrelated.rs"));

    // Check dependency capabilities
    assert!(check_pair_dependency_capability(
        "src/caller.rs",
        "src/unrelated.rs",
        Some(&ingest),
        Some(&interner)
    ));
    assert!(!check_pair_dependency_capability(
        "src/caller.rs",
        "docs/arch.md",
        Some(&ingest),
        Some(&interner)
    ));

    // Create 6 commits across history:
    // Commits 0, 1 in sub-window 0
    // Commits 2, 3 in sub-window 1
    // Commits 4, 5 in sub-window 2
    let commits = vec![
        CommitInfo {
            id: "c0".to_string(),
            timestamp: 600,
            author: "dev@example.com".to_string(),
            changed_files: vec![
                "src/caller.rs".to_string(),
                "src/callee.rs".to_string(),
                "src/unrelated.rs".to_string(),
                "docs/arch.md".to_string(),
            ],
        },
        CommitInfo {
            id: "c1".to_string(),
            timestamp: 500,
            author: "dev@example.com".to_string(),
            changed_files: vec![
                "src/caller.rs".to_string(),
                "src/callee.rs".to_string(),
                "src/unrelated.rs".to_string(),
            ],
        },
        CommitInfo {
            id: "c2".to_string(),
            timestamp: 400,
            author: "dev@example.com".to_string(),
            changed_files: vec![
                "src/caller.rs".to_string(),
                "src/callee.rs".to_string(),
                "src/unrelated.rs".to_string(),
            ],
        },
        CommitInfo {
            id: "c3".to_string(),
            timestamp: 300,
            author: "dev@example.com".to_string(),
            changed_files: vec![
                "src/caller.rs".to_string(),
                "src/callee.rs".to_string(),
                "docs/arch.md".to_string(),
            ],
        },
        CommitInfo {
            id: "c4".to_string(),
            timestamp: 200,
            author: "dev@example.com".to_string(),
            changed_files: vec![
                "src/caller.rs".to_string(),
                "src/callee.rs".to_string(),
                "src/unrelated.rs".to_string(),
                "docs/arch.md".to_string(),
            ],
        },
        CommitInfo {
            id: "c5".to_string(),
            timestamp: 100,
            author: "dev@example.com".to_string(),
            changed_files: vec![
                "src/caller.rs".to_string(),
                "src/callee.rs".to_string(),
                "src/unrelated.rs".to_string(),
            ],
        },
    ];

    let opts = CochangeOptions {
        min_support: 3,
        sub_windows: 3,
        max_commit_files: 100,
    };

    let partners = mine_cochange_for_file(
        "src/caller.rs",
        &commits,
        Some(&coupling),
        Some(&ingest),
        Some(&interner),
        &opts,
    );

    // 1. src/callee.rs changed together in all 6 commits (deg = 1.0, recur = 3)
    // BUT it is statically coupled -> surprising = false!
    let callee = partners
        .iter()
        .find(|p| p.file_path == "src/callee.rs")
        .expect("callee partner");
    assert_eq!(callee.together, 6);
    assert_eq!(callee.deg, 1.0);
    assert_eq!(callee.recur, 3);
    assert!(callee.dep_capable);
    assert!(
        !callee.surprising,
        "callee is statically coupled, must not be surprising"
    );

    // 2. src/unrelated.rs changed together in 5 commits (deg = 5/6 = 0.83, recur = 3)
    // AND has NO static coupling -> surprising = true!
    let unrelated = partners
        .iter()
        .find(|p| p.file_path == "src/unrelated.rs")
        .expect("unrelated partner");
    assert_eq!(unrelated.together, 5);
    assert!((unrelated.deg - 0.833).abs() < 0.01);
    assert_eq!(unrelated.recur, 3);
    assert!(unrelated.dep_capable);
    assert!(
        unrelated.surprising,
        "unrelated has no static link, must be surprising"
    );

    // 3. docs/arch.md changed together in 3 commits
    // BUT markdown is not dependency capable -> dep_capable = false, surprising = false!
    let arch_doc = partners
        .iter()
        .find(|p| p.file_path == "docs/arch.md")
        .expect("arch doc partner");
    assert_eq!(arch_doc.together, 3);
    assert!(!arch_doc.dep_capable);
    assert!(!arch_doc.surprising);

    // Test repo-wide pairs
    let pairs = mine_repo_cochange_pairs(
        &commits,
        Some(&coupling),
        Some(&ingest),
        Some(&interner),
        &opts,
    );

    let pair_caller_unrelated = pairs
        .iter()
        .find(|p| {
            (p.file_a == "src/caller.rs" && p.file_b == "src/unrelated.rs")
                || (p.file_a == "src/unrelated.rs" && p.file_b == "src/caller.rs")
        })
        .expect("caller unrelated pair");
    assert_eq!(pair_caller_unrelated.together, 5);
    assert!(pair_caller_unrelated.surprising);

    let pair_caller_callee = pairs
        .iter()
        .find(|p| {
            (p.file_a == "src/caller.rs" && p.file_b == "src/callee.rs")
                || (p.file_a == "src/callee.rs" && p.file_b == "src/caller.rs")
        })
        .expect("caller callee pair");
    assert_eq!(pair_caller_callee.together, 6);
    assert!(!pair_caller_callee.surprising);
}

#[test]
fn test_cli_cochange_execution_and_xml_output() {
    let interner = Interner::new();
    let ingest = create_test_ingest(&interner);
    let state = McpState::from_ingest(".", ingest, interner);

    let commits = vec![
        CommitInfo {
            id: "c1".to_string(),
            timestamp: 100,
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/caller.rs".to_string(), "src/unrelated.rs".to_string()],
        },
        CommitInfo {
            id: "c2".to_string(),
            timestamp: 200,
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/caller.rs".to_string(), "src/unrelated.rs".to_string()],
        },
        CommitInfo {
            id: "c3".to_string(),
            timestamp: 300,
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/caller.rs".to_string(), "src/unrelated.rs".to_string()],
        },
    ];

    // 1. Probed file execution
    let mut buf = Cursor::new(Vec::new());
    execute_cochange_with(Some("src/caller.rs"), None, &commits, &state, &mut buf)
        .expect("execute cochange");

    let xml = String::from_utf8(buf.into_inner()).unwrap();
    assert!(xml.contains("<f path=\"src/caller.rs\""));
    assert!(xml.contains("<partner path=\"src/unrelated.rs\" together=\"3\""));
    assert!(xml.contains("surprising=\"1\""));

    // 2. Repo-wide execution
    let mut repo_buf = Cursor::new(Vec::new());
    execute_cochange_with(None, None, &commits, &state, &mut repo_buf)
        .expect("execute repo cochange");

    let repo_xml = String::from_utf8(repo_buf.into_inner()).unwrap();
    assert!(repo_xml.contains("<cochange schema=\"ripwire.cochange/v1\" pairs=\"1\">"));
    assert!(repo_xml.contains("<pair a=\"src/caller.rs\" b=\"src/unrelated.rs\" together=\"3\""));
}

#[test]
fn test_mcp_cochange_tool_json_rpc() {
    let interner = Interner::new();
    let ingest = create_test_ingest(&interner);
    let commits = vec![
        CommitInfo {
            id: "c1".to_string(),
            timestamp: 100,
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/caller.rs".to_string(), "src/unrelated.rs".to_string()],
        },
        CommitInfo {
            id: "c2".to_string(),
            timestamp: 200,
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/caller.rs".to_string(), "src/unrelated.rs".to_string()],
        },
        CommitInfo {
            id: "c3".to_string(),
            timestamp: 300,
            author: "dev@example.com".to_string(),
            changed_files: vec!["src/caller.rs".to_string(), "src/unrelated.rs".to_string()],
        },
    ];

    let mut state = Some(McpState::from_ingest(".", ingest, interner).with_commits(commits));

    let req = r#"{"jsonrpc":"2.0","id":42,"method":"tools/call","params":{"name":"cochange","arguments":{"path":"src/caller.rs"}}}"#;
    let resp = dispatch_json_rpc(req, &mut state, ".").expect("reply to cochange");
    let val: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(val["id"], 42);

    let text = val["result"]["content"][0]["text"].as_str().unwrap();
    let data: Value = serde_json::from_str(text).unwrap();
    assert_eq!(data["path"], "src/caller.rs");

    let partners = data["partners"].as_array().expect("partners array");
    assert_eq!(partners.len(), 1);
    assert_eq!(partners[0]["file"], "src/unrelated.rs");
    assert_eq!(partners[0]["together"], 3);
    assert_eq!(partners[0]["surprising"], true);
    assert_eq!(partners[0]["dep_capable"], true);
}
