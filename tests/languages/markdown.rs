use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("Guide", SymKind::Section, None, false),
            sym("Setup", SymKind::Section, Some("Guide"), false),
        ],
        ..Default::default()
    });
}
