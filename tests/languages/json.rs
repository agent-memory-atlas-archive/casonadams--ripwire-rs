use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_json_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "json",
        files: &[(
            "package.json",
            Lang::Json,
            b"{\n  \"name\": \"pkg\",\n  \"scripts\": {\n    \"build\": \"cargo build\",\n    \"nested\": { \"deep_leaf\": true }\n  }\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "name", kind: SymKind::Section, scope: None, has_body_span: false },
            ExpectedSym { name: "scripts", kind: SymKind::Section, scope: None, has_body_span: false },
            ExpectedSym { name: "build", kind: SymKind::Section, scope: None, has_body_span: false },
        ],
        rejected_syms: &["deep_leaf"],
        expected_edges: &[],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    });
}
