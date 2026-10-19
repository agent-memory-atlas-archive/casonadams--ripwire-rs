use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_html_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "html",
        files: &[(
            "index.html",
            Lang::Html,
            b"<html><body><div id=\"app\"><span>text</span></div></body></html>\n",
        )],
        expected_syms: &[sym("html", SymKind::Section, None, false)],
        rejected_syms: &["div", "span"],
        ..Default::default()
    });
}
