use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_toml_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "toml",
        files: &[(
            "Cargo.toml",
            Lang::Toml,
            b"[package]\nname = \"demo\"\n[dependencies]\nserde = { version = \"1.0\" }\n",
        )],
        expected_syms: &[
            ExpectedSym {
                name: "package",
                kind: SymKind::Section,
                scope: None,
                has_body_span: false,
            },
            ExpectedSym {
                name: "name",
                kind: SymKind::Section,
                scope: None,
                has_body_span: false,
            },
            ExpectedSym {
                name: "dependencies",
                kind: SymKind::Section,
                scope: None,
                has_body_span: false,
            },
            ExpectedSym {
                name: "serde",
                kind: SymKind::Section,
                scope: None,
                has_body_span: false,
            },
        ],
        rejected_syms: &["version"],
        expected_edges: &[],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    });
}
