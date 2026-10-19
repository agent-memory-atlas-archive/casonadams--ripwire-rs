use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_yaml_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "yaml",
        files: &[(
            "ci.yaml",
            Lang::Yaml,
            b"<<: *base\njobs:\n  test:\n    steps:\n      - run: cargo test\n",
        )],
        expected_syms: &[
            ExpectedSym {
                name: "jobs",
                kind: SymKind::Section,
                scope: None,
                has_body_span: false,
            },
            ExpectedSym {
                name: "test",
                kind: SymKind::Section,
                scope: None,
                has_body_span: false,
            },
        ],
        rejected_syms: &["<<", "steps", "run"],
        expected_edges: &[],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    });
}
