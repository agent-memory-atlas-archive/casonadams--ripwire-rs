use ripwire_rs::serialize::PageRequest;
use ripwire_rs::{
    CloneIdiom, CloneType, Interner, Lang, LanguageRegistry, find_clones, ingest_in_memory_files,
    render_clones_xml,
};
use std::collections::HashMap;

#[test]
fn test_clone_type1_and_type2_detection() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();

    let code1 = b"fn func_one(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n    let prod = a * b;\n    return sum + prod + 100;\n}\n";
    let code2 = b"fn func_two(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n    let prod = a * b;\n    return sum + prod + 100;\n}\n";
    let code3 = b"fn func_three(x: i32, y: i32) -> i32 {\n    let alpha = x + y;\n    let beta = x * y;\n    return alpha + beta + 100;\n}\n";

    let mut sources = HashMap::new();
    sources.insert("src/a.rs".into(), code1.to_vec());
    sources.insert("src/b.rs".into(), code2.to_vec());
    sources.insert("src/c.rs".into(), code3.to_vec());

    let ingest = ingest_in_memory_files(
        &[
            ("src/a.rs", Lang::Rust, code1),
            ("src/b.rs", Lang::Rust, code2),
            ("src/c.rs", Lang::Rust, code3),
        ],
        &registry,
        &interner,
    );

    let clones = find_clones(&ingest, &interner, 10, Some(&sources));
    assert!(!clones.is_empty(), "Should detect clone group");
    assert_eq!(clones[0].members.len(), 3);
    assert_eq!(clones[0].clone_type, CloneType::Type2);
    let xml = render_clones_xml(&clones, &ingest, &interner, PageRequest::default());
    assert!(xml.contains("<clones schema=\"ripwire-rs.clones/v1\""));
    assert!(xml.contains("<f n=\"func_one\""));
    assert!(xml.contains("<group type=\"2\""));
    assert_eq!(CloneIdiom::None.as_str(), None);
    assert_eq!(
        CloneIdiom::ThresholdLadder.as_str(),
        Some("threshold-ladder")
    );
    assert_eq!(
        CloneIdiom::SwitchNameTable.as_str(),
        Some("switch-name-table")
    );
    assert_eq!(CloneIdiom::BuilderChain.as_str(), Some("builder-chain"));
}

#[test]
fn test_clone_type3_gapped_detection() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();

    let code_base = b"fn process_items(items: &[i32]) -> i32 {\n    let mut total = 0;\n    for &item in items {\n        if item > 0 {\n            total += item * 2;\n        }\n    }\n    total\n}\n";
    let code_near = b"fn process_values(values: &[i32]) -> i32 {\n    let mut total = 0;\n    let bonus = 1;\n    for &elem in values {\n        if elem > 0 {\n            total += elem * 2;\n        }\n    }\n    total + bonus\n}\n";

    let mut sources = HashMap::new();
    sources.insert("src/base.rs".into(), code_base.to_vec());
    sources.insert("src/near.rs".into(), code_near.to_vec());

    let ingest = ingest_in_memory_files(
        &[
            ("src/base.rs", Lang::Rust, code_base),
            ("src/near.rs", Lang::Rust, code_near),
        ],
        &registry,
        &interner,
    );

    let clones = find_clones(&ingest, &interner, 10, Some(&sources));
    assert!(!clones.is_empty(), "Should detect Type-3 near-miss clone");
    let t3 = clones.iter().find(|g| g.clone_type == CloneType::Type3);
    assert!(t3.is_some(), "Expected Type-3 clone");
    let g = t3.unwrap();
    assert_eq!(g.members.len(), 2);
    assert!(g.similarity >= 0.80 && g.similarity < 1.0);
}

#[test]
fn clone_report_honors_limit_and_offset() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let sum = |name: &str| {
        format!(
            "fn {name}(a: i32, b: i32) -> i32 {{\n    let sum = a + b;\n    let prod = a * b;\n    return sum + prod + 100;\n}}\n"
        )
    };
    let sign = |name: &str| {
        format!(
            "fn {name}(v: i32) -> i32 {{\n    if v > 0 {{\n        return v * 2;\n    }}\n    return v - 1;\n}}\n"
        )
    };
    let bodies = [sum("one"), sum("two"), sign("pos"), sign("neg")];
    let paths = ["src/a.rs", "src/b.rs", "src/c.rs", "src/d.rs"];
    let files: Vec<_> = paths
        .iter()
        .zip(&bodies)
        .map(|(p, b)| (*p, Lang::Rust, b.as_bytes()))
        .collect();
    let sources: HashMap<String, Vec<u8>> = paths
        .iter()
        .zip(&bodies)
        .map(|(p, b)| ((*p).into(), b.as_bytes().to_vec()))
        .collect();
    let ingest = ingest_in_memory_files(&files, &registry, &interner);
    let clones = find_clones(&ingest, &interner, 10, Some(&sources));
    assert_eq!(clones.len(), 2);
    let report = |limit, offset| {
        let xml = render_clones_xml(&clones, &ingest, &interner, PageRequest { limit, offset });
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let root = doc.root_element();
        let groups = doc
            .descendants()
            .filter(|n| n.has_tag_name("group"))
            .count();
        (
            groups,
            root.attribute("shown").map(str::to_string),
            root.attribute("has_more").map(str::to_string),
        )
    };
    assert_eq!(report(None, None).0, 2);
    assert_eq!(
        report(Some(1), None),
        (1, Some("1".into()), Some("1".into()))
    );
    assert_eq!(
        report(Some(1), Some(1)),
        (1, Some("1".into()), Some("0".into()))
    );
}
