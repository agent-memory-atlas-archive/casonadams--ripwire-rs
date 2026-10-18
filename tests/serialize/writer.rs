use ripwire_rs::serialize::{MinifyingWriter, XmlWriter, serialize_legend_dict_xml};
use std::io::Write;

#[test]
fn test_minifying_writer_strips_inter_tag_whitespace() {
    let input = "<!-- comment -->\n<root a=\"1\">\n  <child b=\"2\"/>\n  <child c=\"3\">hello</child>\n</root>\n";
    let mut out = Vec::new();
    {
        let mut writer = MinifyingWriter::new(&mut out);
        writer.write_all(input.as_bytes()).unwrap();
    }
    let res = String::from_utf8(out).unwrap();
    assert_eq!(
        res,
        "<!-- comment --><root a=\"1\"><child b=\"2\"/><child c=\"3\">hello</child></root>\n"
    );
}

#[test]
fn test_xml_writer_json_conversion() {
    let input = "<!-- comment -->\n<r schema=\"ripwire.map/v1\" files=\"2\"><f p=\"src/lib.rs\"><s t=\"fn\" n=\"run\"/></s></f></r>\n";
    let mut out = Vec::new();
    {
        let mut writer = XmlWriter::with_format(&mut out, false, true);
        writer.write_all(input.as_bytes()).unwrap();
        writer.flush().unwrap();
    }
    let val: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(val["schema"], "ripwire.map/v1");
    assert_eq!(val["files"], 2);
    assert!(!val["items"].as_array().unwrap().is_empty());
}

#[test]
fn test_xml_writer_strip_comments_and_legend_dict() {
    let input = "<!-- ripwire map schema=ripwire.map/v1 -->\n<r schema=\"ripwire.map/v1\"><f p=\"src/lib.rs\"/></r>\n";
    let mut out = Vec::new();
    {
        let mut writer = XmlWriter::with_options(&mut out, false, false, true);
        writer.write_all(input.as_bytes()).unwrap();
        writer.flush().unwrap();
    }
    let res = String::from_utf8(out).unwrap();
    assert!(!res.contains("<!--"));
    assert!(res.starts_with("<r schema=\"ripwire.map/v1\">"));

    let mut dict_buf = Vec::new();
    serialize_legend_dict_xml(&mut dict_buf).unwrap();
    let dict_str = String::from_utf8(dict_buf).unwrap();
    assert!(dict_str.contains("schema=\"ripwire.legend-dict/v1\""));
    assert!(dict_str.contains("k=\"ripwire.map/v1\""));
    assert!(dict_str.contains("k=\"ccx\""));
    assert!(dict_str.contains("k=\"prov\""));
}
