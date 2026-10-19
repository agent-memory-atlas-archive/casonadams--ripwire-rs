use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_markdown_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "markdown",
        files: &[(
            "docs/GUIDE.md",
            Lang::Markdown,
            b"# Guide\nIntro\n## Setup\nSteps\n",
        )],
        expected_syms: &[
            ExpectedSym {
                name: "Guide",
                kind: SymKind::Section,
                scope: None,
                has_body_span: false,
            },
            ExpectedSym {
                name: "Setup",
                kind: SymKind::Section,
                scope: Some("Guide"),
                has_body_span: false,
            },
        ],
        rejected_syms: &[],
        expected_edges: &[],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    });
}
