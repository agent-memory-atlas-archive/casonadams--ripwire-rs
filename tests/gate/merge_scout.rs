use ripwire_rs::gate::merge_scout::{evaluate_merge_scout, serialize_merge_scout_xml};
use ripwire_rs::{FileInfo, IngestResult, Interner, Lang, SymKind, Symbol};

#[test]
fn test_merge_scout_evaluation_and_serialization() {
    let interner = Interner::new();
    let mut ingest = IngestResult::new();
    let p = interner.intern("src/worker.rs");
    ingest.files.push(FileInfo {
        id: 0,
        path: p,
        lang: Lang::Rust,
        byte_size: 100,
        line_count: 10,
    });
    let sname = interner.intern("feature_a_worker");
    ingest.symbols.push(Symbol::new(
        0,
        sname,
        sname,
        SymKind::Function,
        Lang::Rust,
        0,
        1,
    ));

    let report = evaluate_merge_scout("feature_a,feature_b", &ingest, &interner);
    assert_eq!(report.arms_count, 2);
    assert_eq!(report.pairs.len(), 1);
    assert_eq!(report.landing_order, vec!["feature_a", "feature_b"]);

    let mut out = Vec::new();
    serialize_merge_scout_xml(&report, &mut out).expect("serialize should succeed");
    let xml = String::from_utf8(out).expect("valid utf-8");
    assert!(xml.contains("schema=\"ripwire-rs.merge-scout/v1\""));
    assert!(xml.contains("arms=\"2\""));
    assert!(xml.contains("<arm ref=\"feature_a\""));
    assert!(xml.contains("<arm ref=\"feature_b\""));
    assert!(xml.contains("<pair a=\"feature_a\" b=\"feature_b\" conflicts=\"0\" risks=\"0\"/>"));
    assert!(xml.contains("<landing order=\"feature_a,feature_b\"/>"));
}
