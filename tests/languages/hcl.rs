use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("resource", SymKind::Class, None, false),
            sym("aws_s3_bucket", SymKind::Section, None, false),
            sym("bucket", SymKind::Field, None, false),
        ],
        ..Default::default()
    });
}
