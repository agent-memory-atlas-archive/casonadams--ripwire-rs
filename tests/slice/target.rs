use ripwire_rs::{
    Interner, Lang, LanguageRegistry, SliceFlowDir, execute_slice, ingest_in_memory_files,
    parse_slice_target,
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
fn test_execute_slice_e2e() {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();

    let code =
        b"fn compute(x: i32) -> i32 {\n    let y = x + 1;\n    let z = y * 2;\n    return z;\n}\n";
    let ingest = ingest_in_memory_files(&[("src/lib.rs", Lang::Rust, code)], &registry, &interner);

    let locals_res = execute_slice(
        &ingest,
        &registry,
        &interner,
        "compute",
        SliceFlowDir::Fwd,
        8,
        Some(code),
    )
    .unwrap();
    assert!(locals_res.contains("<slice schema=\"ripwire.slice/v1\" sym=\"compute\""));
    assert!(locals_res.contains("<v n=\"x\""));
    assert!(locals_res.contains("<v n=\"y\""));

    let fwd_res = execute_slice(
        &ingest,
        &registry,
        &interner,
        "compute:x",
        SliceFlowDir::Fwd,
        8,
        Some(code),
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
}
