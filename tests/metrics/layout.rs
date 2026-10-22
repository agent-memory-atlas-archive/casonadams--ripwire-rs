use ripwire_rs::ToXmlString;
use ripwire_rs::metrics::layout::parse_struct_fields;
use ripwire_rs::metrics::{
    FieldLayout, StructLayoutDef, StructLayoutReport, serialize_field_affinity_xml,
};

#[test]
fn test_layout_parsing() {
    let body = "struct Foo {\n  uint64_t a;\n  uint32_t b;\n  uint8_t c;\n};";
    let fields = parse_struct_fields(body);
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].offset, 0);
    assert_eq!(fields[1].offset, 8);
    assert_eq!(fields[2].offset, 12);
}

#[test]
fn test_serialize_layout_and_affinity() {
    let def = StructLayoutDef {
        path: "src/test.rs".into(),
        line: 10,
        agg_kind: "struct",
        fields: vec![
            FieldLayout {
                name: "a".into(),
                ty: "u64".into(),
                size: 8,
                align: 8,
                offset: 0,
            },
            FieldLayout {
                name: "b".into(),
                ty: "u32".into(),
                size: 4,
                align: 4,
                offset: 8,
            },
        ],
    };
    let rep = StructLayoutReport {
        sym: "Foo".into(),
        found: true,
        defs: vec![def],
    };
    assert!(
        rep.to_xml_string()
            .contains("schema=\"ripwire-rs.layout/v1\"")
    );

    let mut buf2 = Vec::new();
    serialize_field_affinity_xml(&rep, ".", &mut buf2).unwrap();
    assert!(
        String::from_utf8(buf2)
            .unwrap()
            .contains("schema=\"ripwire-rs.field-affinity/v1\"")
    );
}
