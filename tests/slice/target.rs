use ripwire_rs::slice::{SliceContext, SliceRequest, query_slice};
use ripwire_rs::{
    Interner, Lang, LanguageRegistry, SliceFlowDir, ingest_in_memory_files, parse_slice_target,
};

#[test]
fn test_slice_target_parsing() {
    assert_eq!(parse_slice_target("foo:bar"), ("foo", Some("bar")));
    assert_eq!(
        parse_slice_target("src/main.rs::foo:bar"),
        ("src/main.rs::foo", Some("bar"))
    );
    assert_eq!(parse_slice_target("foo"), ("foo", None));
    assert_eq!(parse_slice_target("foo::bar"), ("foo::bar", None));
}

#[test]
fn test_execute_slice_locals() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();

    let code =
        b"fn compute(x: i32) -> i32 {\n    let y = x + 1;\n    let z = y * 2;\n    return z;\n}\n";
    let ingest = ingest_in_memory_files(&[("src/lib.rs", Lang::Rust, code)], &registry, &interner);

    let sources = [("src/lib.rs", code.as_slice())];
    let locals_res = query_slice(
        SliceRequest {
            root: "/virtual/project",
            at: None,
            target: "compute",
            flow: SliceFlowDir::Fwd,
            max_depth: 8,
        },
        SliceContext {
            ingest: &ingest,
            registry: &registry,
            interner: &interner,
        },
        &sources,
    )
    .unwrap();
    assert!(locals_res.contains("<slice schema=\"ripwire-rs.slice/v1\" sym=\"compute\""));
    assert!(locals_res.contains("<v n=\"x\""));
    assert!(locals_res.contains("<v n=\"y\""));
    assert!(!locals_res.contains(" at="));

    let stamped = query_slice(
        SliceRequest {
            root: "/virtual/<repo>",
            at: Some("rev\"&"),
            target: "compute",
            flow: SliceFlowDir::Fwd,
            max_depth: 1,
        },
        SliceContext {
            ingest: &ingest,
            registry: &registry,
            interner: &interner,
        },
        &sources,
    )
    .unwrap();
    assert!(stamped.contains("root=\"/virtual/&lt;repo&gt;\""));
    assert!(stamped.contains(" at=\"rev&quot;&amp;\""));
}

#[test]
fn test_execute_slice_e2e() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();

    let code =
        b"fn compute(x: i32) -> i32 {\n    let y = x + 1;\n    let z = y * 2;\n    return z;\n}\n";
    let ingest = ingest_in_memory_files(&[("src/lib.rs", Lang::Rust, code)], &registry, &interner);

    let sources = [("src/lib.rs", code.as_slice())];
    let fwd_res = query_slice(
        SliceRequest {
            root: "/virtual/project",
            at: None,
            target: "compute:x",
            flow: SliceFlowDir::Fwd,
            max_depth: 8,
        },
        SliceContext {
            ingest: &ingest,
            registry: &registry,
            interner: &interner,
        },
        &sources,
    )
    .unwrap();
    assert!(fwd_res.contains("flow=\"fwd\""));
    assert!(fwd_res.contains(
        "<s l=\"1\" k=\"param\" v=\"x\" d=\"0\"><![CDATA[fn compute(x: i32) -> i32 {]]></s>"
    ));
    assert!(fwd_res.contains(
        "<s l=\"2\" k=\"read\" v=\"x\" d=\"1\" from=\"1\" rd=\"1\"><![CDATA[let y = x + 1;]]></s>"
    ));
    assert!(fwd_res.contains(
        "<s l=\"3\" k=\"read\" v=\"y\" d=\"2\" from=\"2\" rd=\"2\"><![CDATA[let z = y * 2;]]></s>"
    ));
    assert!(fwd_res.contains(
        "<s l=\"4\" k=\"read\" v=\"z\" d=\"3\" from=\"3\" rd=\"3\"><![CDATA[return z;]]></s>"
    ));
    assert!(fwd_res.contains("root=\"/virtual/project\""));
    assert!(!fwd_res.contains(" at="));
    let shallow = query_slice(
        SliceRequest {
            root: "/another/root",
            at: None,
            target: "compute:x",
            flow: SliceFlowDir::Fwd,
            max_depth: 1,
        },
        SliceContext {
            ingest: &ingest,
            registry: &registry,
            interner: &interner,
        },
        &sources,
    )
    .unwrap();
    assert!(shallow.contains("root=\"/another/root\""));
    assert!(shallow.contains("depth=\"1\""));
    assert!(!shallow.contains("v=\"y\" d=\"2\""));
    let missing_sources: [(&str, &[u8]); 0] = [];
    let missing = query_slice(
        SliceRequest {
            root: "/virtual/project",
            at: None,
            target: "compute:x",
            flow: SliceFlowDir::Fwd,
            max_depth: 8,
        },
        SliceContext {
            ingest: &ingest,
            registry: &registry,
            interner: &interner,
        },
        &missing_sources,
    )
    .unwrap_err();
    assert!(missing.contains("cannot read file src/lib.rs"));
    let missing_target = query_slice(
        SliceRequest {
            root: "/virtual/project",
            at: None,
            target: "absent",
            flow: SliceFlowDir::Fwd,
            max_depth: 8,
        },
        SliceContext {
            ingest: &ingest,
            registry: &registry,
            interner: &interner,
        },
        &sources,
    )
    .unwrap_err();
    assert_eq!(missing_target, "symbol not found: absent");
    let stamped = query_slice(
        SliceRequest {
            root: "/virtual/<repo>",
            at: Some("rev\"&"),
            target: "compute:x",
            flow: SliceFlowDir::Fwd,
            max_depth: 1,
        },
        SliceContext {
            ingest: &ingest,
            registry: &registry,
            interner: &interner,
        },
        &sources,
    )
    .unwrap();
    assert!(stamped.contains("root=\"/virtual/&lt;repo&gt;\""));
    assert!(stamped.contains(" at=\"rev&quot;&amp;\""));
}

#[test]
fn selects_definition_containing_variable_across_injected_files() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let first = b"fn compute(other: i32) -> i32 { other }\n";
    let second = b"fn compute(wanted: i32) -> i32 { wanted }\n";
    let ingest = ingest_in_memory_files(
        &[
            ("a.rs", Lang::Rust, first.as_slice()),
            ("b.rs", Lang::Rust, second.as_slice()),
        ],
        &registry,
        &interner,
    );
    let sources = [("a.rs", first.as_slice()), ("b.rs", second.as_slice())];
    let report = query_slice(
        SliceRequest {
            root: "/virtual/repo",
            at: None,
            target: "compute:wanted",
            flow: SliceFlowDir::Fwd,
            max_depth: 4,
        },
        SliceContext {
            ingest: &ingest,
            registry: &registry,
            interner: &interner,
        },
        &sources,
    )
    .unwrap();
    assert!(report.contains("p=\"b.rs:1\""));
    assert!(report.contains("k=\"param\" v=\"wanted\""));

    let second_only = [("b.rs", second.as_slice())];
    let report = query_slice(
        SliceRequest {
            root: "/virtual/repo",
            at: None,
            target: "compute:wanted",
            flow: SliceFlowDir::Fwd,
            max_depth: 4,
        },
        SliceContext {
            ingest: &ingest,
            registry: &registry,
            interner: &interner,
        },
        &second_only,
    )
    .unwrap();
    assert!(report.contains("p=\"b.rs:1\""));

    let no_sources: [(&str, &[u8]); 0] = [];
    let error = query_slice(
        SliceRequest {
            root: "/virtual/repo",
            at: None,
            target: "compute:wanted",
            flow: SliceFlowDir::Fwd,
            max_depth: 4,
        },
        SliceContext {
            ingest: &ingest,
            registry: &registry,
            interner: &interner,
        },
        &no_sources,
    )
    .unwrap_err();
    assert!(error.contains("cannot read file"));
}
