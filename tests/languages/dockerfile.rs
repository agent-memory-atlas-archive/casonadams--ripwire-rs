use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_dockerfile_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "dockerfile",
        files: &[(
            "Dockerfile",
            Lang::Dockerfile,
            b"FROM rust:1.85 AS builder\nRUN cargo build --release\n",
        )],
        expected_syms: &[ExpectedSym {
            name: "builder",
            kind: SymKind::Class,
            scope: None,
            has_body_span: false,
        }],
        rejected_syms: &[],
        expected_edges: &[],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    });
}
