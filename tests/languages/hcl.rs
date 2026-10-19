use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_hcl_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "hcl",
        files: &[(
            "infra/main.tf",
            Lang::Hcl,
            b"resource \"aws_s3_bucket\" \"primary\" {\n  bucket = format(\"%s\", var.name)\n}\n",
        )],
        expected_syms: &[
            ExpectedSym {
                name: "resource",
                kind: SymKind::Class,
                scope: None,
                has_body_span: false,
            },
            ExpectedSym {
                name: "aws_s3_bucket",
                kind: SymKind::Section,
                scope: None,
                has_body_span: false,
            },
            ExpectedSym {
                name: "bucket",
                kind: SymKind::Field,
                scope: None,
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
