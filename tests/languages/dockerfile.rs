use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
        expected_syms: &[sym("builder", SymKind::Class, None, false)],
        ..Default::default()
    });
}
