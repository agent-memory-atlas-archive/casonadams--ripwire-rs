use ripwire_rs::git::cochange::{CoGroup, CoPair, CoPartner};
use ripwire_rs::git::history::CommitInfo;
use ripwire_rs::query::safety::*;
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, ToXmlString, ingest_in_memory_files};

#[test]
fn test_quality_baseline_from_facts_clean() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let files = [(
        "src/lib.rs",
        Lang::Rust,
        b"pub fn clean_func() {}\n".as_slice(),
    )];
    let ingest = ingest_in_memory_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let context = SafetyContext {
        root: "pure-facts",
        ingest: &ingest,
        graph: &graph,
        interner: &interner,
    };

    let snap = baseline_from_facts(&context, "sha_clean_123", None, false).unwrap();
    assert_eq!(snap.head_sha, "sha_clean_123");
    assert!(!snap.dirty);
    assert_eq!(snap.absorbed, 0);
}

#[test]
fn test_quality_baseline_from_facts_dirty_policy() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let files = [("src/lib.rs", Lang::Rust, b"pub fn sample() {}\n".as_slice())];
    let ingest = ingest_in_memory_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let context = SafetyContext {
        root: "pure-facts",
        ingest: &ingest,
        graph: &graph,
        interner: &interner,
    };

    let err = baseline_from_facts(
        &context,
        "sha_dirty_456",
        Some((3, "complexity sample src/lib.rs:1 was=2 now=8")),
        false,
    )
    .unwrap_err();
    assert!(err.to_string().contains("3 gating finding(s)"));
    assert!(err.to_string().contains("--allow-dirty"));

    let snap = baseline_from_facts(
        &context,
        "sha_dirty_456",
        Some((3, "complexity sample src/lib.rs:1 was=2 now=8")),
        true,
    )
    .unwrap();
    assert_eq!(snap.head_sha, "sha_dirty_456");
    assert!(snap.dirty);
    assert_eq!(snap.absorbed, 3);
}

#[test]
fn test_cochange_printers_and_formatting() {
    let mut out = Vec::new();
    let partners = [
        CoPartner {
            file_path: "src/b.rs".to_string(),
            together: 5,
            deg: 0.85,
            deg_rev: 0.80,
            recur: 2,
            surprising: true,
            dep_capable: true,
        },
        CoPartner {
            file_path: "src/c.rs".to_string(),
            together: 2,
            deg: 0.40,
            deg_rev: 0.30,
            recur: 1,
            surprising: false,
            dep_capable: false,
        },
    ];
    print_cochange_partners("src/a.rs", &partners, &mut out).unwrap();
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("<f path=\"src/a.rs\" partners=\"2\">"));
    assert!(text.contains("<partner path=\"src/b.rs\" together=\"5\" deg=\"0.85\" deg_rev=\"0.80\" recur=\"2\" surprising=\"1\" />"));
    assert!(text.contains("<partner path=\"src/c.rs\" together=\"2\" deg=\"0.40\" deg_rev=\"0.30\" recur=\"1\" dep_capable=\"0\" />"));

    out.clear();
    let pairs = [
        CoPair {
            file_a: "src/one.rs".to_string(),
            file_b: "src/two.rs".to_string(),
            together: 7,
            deg: 0.9,
            conf_ab: 0.8,
            conf_ba: 0.7,
            driver: "a",
            recur: 3,
            surprising: false,
            dep_capable: true,
        },
        CoPair {
            file_a: "src/alpha.rs".to_string(),
            file_b: "src/beta.rs".to_string(),
            together: 3,
            deg: 0.5,
            conf_ab: 0.4,
            conf_ba: 0.3,
            driver: "b",
            recur: 1,
            surprising: true,
            dep_capable: true,
        },
    ];
    print_cochange_pairs_paged("root", &pairs, Some(1), Some(0), &mut out).unwrap();
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("<cochange schema=\"ripwire-rs.cochange/v1\" pairs=\"2\""));
    assert!(text.contains("<pair a=\"src/one.rs\" b=\"src/two.rs\" together=\"7\" deg=\"0.90\" conf_ab=\"0.80\" conf_ba=\"0.70\" driver=\"a\" recur=\"3\" surprising=\"0\"/>"));
    assert!(!text.contains("src/alpha.rs"));

    out.clear();
    let groups = [CoGroup {
        core: "src/x.rs".to_string(),
        partners: vec!["src/y.rs".to_string()],
        max_together: 4,
        max_deg: 0.75,
    }];
    print_cochange_groups_paged("root", &groups, None, None, &mut out).unwrap();
    let text = std::str::from_utf8(&out).unwrap();
    assert!(
        text.contains("<cochange-groups schema=\"ripwire-rs.cochange-groups/v1\" groups=\"1\"")
    );
    assert!(text.contains(
        "<group core=\"src/x.rs\" partners=\"src/y.rs\" size=\"2\" together=\"4\" deg=\"0.75\"/>"
    ));
    assert!(text.contains("src/x.rs"));
    assert!(text.contains("src/y.rs"));
}

#[test]
fn test_execute_cochange_and_hotspots_with_injected_history() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let files = [
        (
            "src/a.rs",
            Lang::Rust,
            b"pub fn a(x: bool) { if x {} }\n".as_slice(),
        ),
        (
            "src/b.rs",
            Lang::Rust,
            b"pub fn b(y: bool) { if y {} }\n".as_slice(),
        ),
    ];
    let ingest = ingest_in_memory_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let s_ctx = SafetyContext {
        root: "root",
        ingest: &ingest,
        graph: &graph,
        interner: &interner,
    };

    let commits: Vec<_> = (0..4)
        .map(|index| CommitInfo {
            id: format!("commit{index}"),
            timestamp: 1_700_000_000 - index * 3_000_000,
            author: "Ada".to_string(),
            changed_files: vec!["src/a.rs".to_string(), "src/b.rs".to_string()],
        })
        .collect();

    let mut out = Vec::new();
    execute_hotspots_with(Some("2023-01-01"), &commits, &s_ctx, &mut out).unwrap();
    let text = std::str::from_utf8(&out).unwrap();
    let hotspots = roxmltree::Document::parse(text).unwrap();
    for path in ["src/a.rs", "src/b.rs"] {
        let row = hotspots
            .descendants()
            .find(|node| node.attribute("p") == Some(path))
            .unwrap();
        assert_eq!(row.attribute("churn"), Some("4"));
    }

    out.clear();
    execute_cochange_with(
        Some("src/a.rs"),
        Some("2023-01-01"),
        &commits,
        &s_ctx,
        &mut out,
    )
    .unwrap();
    let text = std::str::from_utf8(&out).unwrap();
    let partners = roxmltree::Document::parse(text).unwrap();
    let paths: Vec<_> = partners
        .descendants()
        .filter(|node| node.has_tag_name("partner"))
        .map(|node| node.attribute("path").unwrap())
        .collect();
    assert_eq!(paths, ["src/b.rs"]);
}

#[test]
fn test_execute_edit_check_and_affected_consumer_facts() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let files = [(
        "src/lib.rs",
        Lang::Rust,
        b"pub fn target_sym() {}\nfn caller() { target_sym(); }\n".as_slice(),
    )];
    let ingest = ingest_in_memory_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let s_ctx = SafetyContext {
        root: "root",
        ingest: &ingest,
        graph: &graph,
        interner: &interner,
    };

    let mut out = Vec::new();
    let req = EditCheckRequest {
        sym_name: "target_sym",
        head_contract: None,
        page: None,
        at_stamp: Some("head_commit_123"),
    };
    write_edit_check_from_facts(&req, &s_ctx, &mut out).unwrap();
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("<edit-check schema=\"ripwire-rs.edit-check/v1\""));
    assert!(text.contains("sym=\"target_sym\""));
    assert!(text.contains("callers=\"1\""));
    assert!(text.contains("at=\"head_commit_123\""));

    out.clear();
    let sources: [(&str, &[u8]); 1] = [(
        "src/lib.rs",
        b"pub fn target_sym() {}\nfn caller() { target_sym(); }\n",
    )];
    let report = affected_from_sources("src/lib.rs", &s_ctx, &sources);
    report.write_xml(&mut out).unwrap();
    let text = std::str::from_utf8(&out).unwrap();
    assert!(text.contains("<affected schema=\"ripwire-rs.affected/v1\""));
    assert!(report.tests.is_empty());
}

#[test]
fn test_situational_report_consumer_facts() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let files = [(
        "src/lib.rs",
        Lang::Rust,
        b"pub fn core_item() {}\n".as_slice(),
    )];
    let ingest = ingest_in_memory_files(&files, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    let s_ctx = SafetyContext {
        root: "root",
        ingest: &ingest,
        graph: &graph,
        interner: &interner,
    };

    let changed_files = vec!["src/lib.rs".to_string()];
    let report = SituationalReport {
        root: "root",
        files: &changed_files,
        commits: None,
        context: s_ctx,
    };

    let mut out = Vec::new();
    report.write_xml(&mut out).unwrap();
    let xml = std::str::from_utf8(&out).unwrap();
    assert!(xml.contains("<situ schema=\"ripwire-rs.situ/v1\""));
    assert!(xml.contains("<summary files=\"1\""));
    assert!(xml.contains("<changed path=\"src/lib.rs\"/>"));
}
