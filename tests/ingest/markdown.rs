use ripwire_rs::languages::markdown::extract::extract_markdown;
use ripwire_rs::{Lang, LanguageRegistry};

#[test]
fn test_markdown_setext_atx_and_links() {
    let reg = LanguageRegistry::new().unwrap();
    let (language, _) = reg.get(Lang::Markdown).unwrap();
    let md = b"Main Title\n==========\n\n## Hello World ##\r\n\nSub Section\n-----------\n\nHere is a [link](#sub-section).\n";
    let res = extract_markdown(md, "docs/guide.md", language);
    assert!(res.defs.iter().any(|d| d.name == "Main Title"));
    assert!(res.defs.iter().any(|d| d.name == "Hello World"));
    assert!(res.defs.iter().any(|d| d.name == "Sub Section"));
    assert!(!res.refs.is_empty());
    assert_eq!(res.refs[0].callee_name, "sub-section");
}
