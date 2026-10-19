use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
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
        expected_syms: &[ExpectedSym {
            name: "html",
            kind: SymKind::Section,
            scope: None,
            has_body_span: false,
        }],
        rejected_syms: &["div", "span"],
        expected_edges: &[],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    });
}
