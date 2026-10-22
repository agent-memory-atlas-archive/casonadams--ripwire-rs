use ripwire_rs::commands::common::edit::{resolve_edit_spec, resolve_edit_spec_with_reader};
use ripwire_rs::commands::common::ladder::execute_expand_or_outline_compressed;
use ripwire_rs::commands::doc_drift::{DOC_DRIFT, DocDriftArgs};
use ripwire_rs::commands::flags_report::{FLAGS, FlagsArgs};
use ripwire_rs::commands::flip::{FLIP, FlipArgs, render_flip_xml};
use ripwire_rs::commands::handoff::build_handoff_verified_rows;
use ripwire_rs::commands::mentions::{MENTIONS, MentionsArgs};
use ripwire_rs::commands::notes::{NOTES, NotesArgs, render_notes_xml};
use ripwire_rs::commands::plan_lint::{PLAN_LINT, PlanLintArgs, render_plan_lint_xml};
use ripwire_rs::commands::skipped::render_skipped_census_xml;
use ripwire_rs::commands::whereis::{WHEREIS, WhereisArgs};
use ripwire_rs::crawler::{CrawlerOptions, SkippedCensus};
use ripwire_rs::query::detail::skeletonize_body;
use ripwire_rs::query::doc_tools::{WhereisFacts, whereis_from_sources_with_facts};
use ripwire_rs::serialize::{PageRequest, serialize_whereis_xml};
use ripwire_rs::{IngestResult, Interner, Lang, LanguageRegistry, ingest_in_memory_files};

fn render_whereis_xml<W: std::io::Write>(
    val: &serde_json::Value,
    listing: Option<&str>,
    full_text: bool,
    out: &mut W,
) -> std::io::Result<()> {
    let mut val = val.clone();
    if let Some(listing) = listing {
        val["listing"] = listing.into();
    }
    if full_text {
        val["full_text"] = true.into();
    }
    serialize_whereis_xml(&val, out)
}

#[test]
fn test_skeletonize_body() {
    let body = "fn run() {\n    if x {\n        let a = 1;\n        let b = 2;\n    }\n}";
    let skel = skeletonize_body(body);
    assert!(skel.contains("..."));
    assert!(!skel.contains("let a = 1"));
    let interner = Interner::new();
    let ingest = IngestResult::new();
    let mut buf = Vec::new();
    assert!(
        execute_expand_or_outline_compressed(
            ".", "missing", false, false, &ingest, &interner, &mut buf
        )
        .is_err()
    );
    assert!(
        execute_expand_or_outline_compressed(
            ".", "missing", true, false, &ingest, &interner, &mut buf
        )
        .is_err()
    );
}

#[test]
fn test_handoff_xml_branches() {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let fixtures: &[(&str, Lang, &[u8])] = &[("src/lib.rs", Lang::Rust, b"pub fn overview() {}\n")];
    let ingest = ingest_in_memory_files(fixtures, &reg, &interner);
    let empty_rows = build_handoff_verified_rows(&[], &ingest, &interner);
    assert!(empty_rows.contains("src/lib.rs"));
    let ch_rows = build_handoff_verified_rows(&["src/lib.rs".to_string()], &ingest, &interner);
    assert!(ch_rows.contains("overview"));
}

#[test]
fn test_resolve_edit_spec_all_branches() {
    // Inline spec with colon
    let (sym, payload) = resolve_edit_spec("compute:let y = 2;", None).unwrap();
    assert_eq!(sym, "compute");
    assert_eq!(payload, "let y = 2;");

    // Read from stdin when payload_override is "-"
    let stdin_cur = std::io::Cursor::new(b"stdin payload content");
    let (sym2, payload2) = resolve_edit_spec_with_reader("compute", Some("-"), stdin_cur).unwrap();
    assert_eq!(sym2, "compute");
    assert_eq!(payload2, "stdin payload content");

    // Literal string fallback when payload_override is not a readable file
    let stdin_cur2 = std::io::Cursor::new(b"");
    let (sym3, payload3) =
        resolve_edit_spec_with_reader("compute", Some("inline_override_text"), stdin_cur2).unwrap();
    assert_eq!(sym3, "compute");
    assert_eq!(payload3, "inline_override_text");

    // None payload
    let stdin_cur3 = std::io::Cursor::new(b"");
    let (sym4, payload4) = resolve_edit_spec_with_reader("compute", None, stdin_cur3).unwrap();
    assert_eq!(sym4, "compute");
    assert_eq!(payload4, "");
}

#[test]
fn test_render_notes_xml() {
    let mut buf = Vec::new();
    render_notes_xml("note alpha\n\nnote beta\n", &mut buf).unwrap();
    let out = std::str::from_utf8(&buf).unwrap();
    assert!(out.contains("ripwire-rs.notes/v1"));
    assert!(out.contains("<note text=\"note alpha\"/>"));
    assert!(out.contains("<note text=\"note beta\"/>"));
}

#[test]
fn test_render_plan_lint_xml() {
    let mut buf = Vec::new();
    render_plan_lint_xml(
        "PLAN.md",
        "# Heading 1\n## Heading 2\n- [ ] task 1\n- [x] task 2\n",
        &mut buf,
    )
    .unwrap();
    let out = std::str::from_utf8(&buf).unwrap();
    assert!(out.contains("ripwire-rs.plan-lint/v1"));
    assert!(out.contains("headings=\"2\""));
    assert!(out.contains("tasks=\"2\""));
    assert!(out.contains("ok=\"1\""));

    buf.clear();
    render_plan_lint_xml("EMPTY.md", "plain text without headings", &mut buf).unwrap();
    let empty_out = std::str::from_utf8(&buf).unwrap();
    assert!(empty_out.contains("headings=\"0\""));
    assert!(empty_out.contains("ok=\"0\""));
}

#[test]
fn test_render_flip_xml() {
    let flags_val = serde_json::json!({
        "flags": [
            { "name": "gate_deploy", "path": "src/deploy.rs", "line": 42 },
            { "name": "other_flag", "path": "src/other.rs", "line": 10 }
        ]
    });
    let mut buf = Vec::new();
    render_flip_xml("gate_deploy", ".", &flags_val, &mut buf).unwrap();
    let out = std::str::from_utf8(&buf).unwrap();
    assert!(out.contains("ripwire-rs.flip/v1"));
    assert!(out.contains("<site p=\"src/deploy.rs\" l=\"42\" gate=\"gate_deploy\"/>"));
    assert!(!out.contains("other_flag"));
}

#[test]
fn test_render_skipped_census_xml() {
    let mut census = SkippedCensus::default();
    census
        .oversize
        .push(("large<&.rs".to_string(), 5000, "rs".to_string()));
    census
        .unsupported
        .push(("data&.bin".to_string(), 120, "bin".to_string()));
    census.unindexed_exts.push(("bin".to_string(), 1));
    let opts = CrawlerOptions::default();

    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let fixtures: &[(&str, Lang, &[u8])] = &[("src/lib.rs", Lang::Rust, b"pub fn run() {}\n")];
    let ingest = ingest_in_memory_files(fixtures, &reg, &interner);

    let mut buf = Vec::new();
    render_skipped_census_xml(
        "workspace<&",
        &census,
        &opts,
        &ingest,
        PageRequest::default(),
        &mut buf,
    )
    .unwrap();
    let out = std::str::from_utf8(&buf).unwrap();
    let report = roxmltree::Document::parse(out).unwrap();
    assert_eq!(report.root_element().attribute("root"), Some("workspace<&"));
    let paths: Vec<_> = report
        .descendants()
        .filter(|node| node.has_tag_name("f"))
        .map(|node| (node.attribute("p"), node.attribute("why")))
        .collect();
    assert_eq!(
        paths,
        [
            (Some("large<&.rs"), Some("oversize")),
            (Some("data&.bin"), Some("unsupported-ext"))
        ]
    );
}

#[test]
fn test_render_skipped_census_xml_honors_page_window() {
    let mut census = SkippedCensus::default();
    for i in 0..3 {
        census
            .oversize
            .push((format!("big{i}.rs"), 5000, "rs".to_string()));
    }
    let opts = CrawlerOptions::default();
    let ingest = IngestResult::new();
    let page = PageRequest {
        limit: Some(1),
        offset: Some(1),
    };
    let mut buf = Vec::new();
    render_skipped_census_xml(".", &census, &opts, &ingest, page, &mut buf).unwrap();
    let out = std::str::from_utf8(&buf).unwrap();
    assert_eq!(out.matches("why=\"oversize\"").count(), 1);
    assert!(out.contains("big1.rs"));
}

#[test]
fn test_execute_doc_search_query_all_branches() {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let file_bytes = b"pub fn overview() {}\n";
    let fixtures: &[(&str, Lang, &[u8])] = &[("src/lib.rs", Lang::Rust, file_bytes)];
    let ingest = ingest_in_memory_files(fixtures, &reg, &interner);
    let sources: &[(&str, &[u8])] = &[("src/lib.rs", file_bytes)];

    // Whereis pure renderer with acquired-facts fixtures (zero I/O)
    let facts = WhereisFacts {
        history: Some(serde_json::json!({
            "probed": 1,
            "head": "abc1234",
            "fate": {
                "v": "removed",
                "commit": "def5678",
                "date": "2026-10-20",
                "subject": "remove overview symbol"
            }
        })),
        head_date: Some("2026-10-21".to_string()),
        at_stamp: Some("abc1234".to_string()),
        branch_rows: vec![serde_json::json!({
            "ref": "feature-branch",
            "tip": "feat123",
            "date": "2026-10-20",
            "symbol": "overview",
            "kind": "def",
            "file": "src/lib.rs",
            "path": "src/lib.rs",
            "line": 1,
            "text": "pub fn overview() {}",
            "on_head": false,
        })],
        refs_scanned: 1,
    };
    let whereis_val =
        whereis_from_sources_with_facts(&sources, &ingest, &interner, "overview", &facts);
    let mut buf = Vec::new();
    render_whereis_xml(&whereis_val, None, false, &mut buf).unwrap();
    let whereis_xml = std::str::from_utf8(&buf).unwrap();
    assert!(whereis_xml.contains("ripwire-rs.whereis/v1"));
    assert!(whereis_xml.contains("feature-branch"));
    assert!(whereis_xml.contains("fate sym=\"overview\" v=\"removed\""));
    assert!((DOC_DRIFT.selected)(&DocDriftArgs {
        doc_drift: true,
        ..Default::default()
    }));
    assert!((FLAGS.selected)(&FlagsArgs { flags: true }));
    assert!((MENTIONS.selected)(&MentionsArgs {
        mentions: Some("overview".to_string()),
    }));
    assert!((WHEREIS.selected)(&WhereisArgs {
        whereis: Some("overview".to_string()),
        ..Default::default()
    }));
    assert!(!(DOC_DRIFT.selected)(&DocDriftArgs::default()));
    assert!(!(FLAGS.selected)(&FlagsArgs::default()));
    assert!(!(MENTIONS.selected)(&MentionsArgs::default()));
    assert!(!(WHEREIS.selected)(&WhereisArgs::default()));
}

#[test]
fn test_render_whereis_xml_branch_and_history_consumer() {
    let facts = WhereisFacts {
        history: Some(serde_json::json!({
            "probed": 1,
            "head": "c0ffee",
            "fate": {
                "v": "removed",
                "commit": "1234567",
                "date": "2026-10-19",
                "subject": "remove target symbol"
            }
        })),
        head_date: Some("2026-10-21".to_string()),
        at_stamp: Some("c0ffee".to_string()),
        branch_rows: vec![serde_json::json!({
            "ref": "alt-branch",
            "tip": "7654321",
            "date": "2026-10-20",
            "symbol": "target",
            "kind": "ref",
            "file": "src/alt.rs",
            "path": "src/alt.rs",
            "line": 10,
            "text": "call target();",
            "on_head": false,
        })],
        refs_scanned: 2,
    };
    let ingest = IngestResult::new();
    let interner = Interner::new();
    let sources: [(&str, &[u8]); 0] = [];
    let val = whereis_from_sources_with_facts(&sources, &ingest, &interner, "target", &facts);
    let mut buf = Vec::new();
    render_whereis_xml(&val, Some("all"), true, &mut buf).unwrap();
    let out = std::str::from_utf8(&buf).unwrap();
    assert!(out.contains("ripwire-rs.whereis/v1"));
    assert!(out.contains("alt-branch"));
    assert!(out.contains("call target();"));
    assert!(out.contains("fate sym=\"target\" v=\"removed\""));
    assert!(out.contains("head=\"c0ffee\""));
}

#[test]
fn test_session_plugins_render_in_memory_content() {
    let mut buf = Vec::new();
    render_notes_xml("pinned in-memory note", &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("pinned in-memory note"));

    buf.clear();
    render_plan_lint_xml("PLAN.md", "# Target Plan\n- [ ] step 1\n", &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("headings=\"1\""));

    buf.clear();
    let flags_val = serde_json::json!({ "flags": [] });
    render_flip_xml("nonexistent_gate", ".", &flags_val, &mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("flip"));

    assert!((NOTES.selected)(&NotesArgs { notes: true }));
    assert!((FLIP.selected)(&FlipArgs {
        flip: Some("nonexistent_gate".to_string()),
    }));
    assert!((PLAN_LINT.selected)(&PlanLintArgs {
        plan_lint: Some("PLAN.md".to_string()),
    }));
    assert!(!(NOTES.selected)(&NotesArgs::default()));
    assert!(!(FLIP.selected)(&FlipArgs::default()));
    assert!(!(PLAN_LINT.selected)(&PlanLintArgs::default()));
}
