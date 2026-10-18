use ripwire_rs::serialize::XmlEscapeExt;
use ripwire_rs::{escape_xml_str, escape_xml_write};
use std::borrow::Cow;

#[test]
fn test_escape_clean_string() {
    let clean = "clean_identifier_123";
    let res = escape_xml_str(clean);
    assert!(matches!(res, Cow::Borrowed(_)));
    assert_eq!(res, clean);

    let mut buf = Vec::new();
    escape_xml_write(&mut buf, clean).unwrap();
    assert_eq!(std::str::from_utf8(&buf).unwrap(), clean);
}

#[test]
fn test_escape_special_entities() {
    let text = "<tag name=\"foo\" key='bar' flag=\"a & b\">";
    let escaped = escape_xml_str(text);
    assert!(matches!(escaped, Cow::Owned(_)));
    assert_eq!(
        escaped,
        "&lt;tag name=&quot;foo&quot; key=&apos;bar&apos; flag=&quot;a &amp; b&quot;&gt;"
    );

    let mut buf = Vec::new();
    escape_xml_write(&mut buf, text).unwrap();
    assert_eq!(std::str::from_utf8(&buf).unwrap(), escaped.as_ref());
}

#[test]
fn test_escape_controls_and_c0() {
    let text = "line1\tline2\nline3\rline4\x01bell\x1f";
    let escaped = escape_xml_str(text);
    assert_eq!(escaped, "line1&#9;line2&#10;line3&#13;line4 bell ");
    assert_eq!(
        text.xml_escaped().to_string(),
        "line1&#9;line2&#10;line3&#13;line4 bell "
    );
}
