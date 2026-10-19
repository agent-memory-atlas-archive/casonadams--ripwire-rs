use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("package", SymKind::Section, None, false),
            sym("name", SymKind::Section, None, false),
            sym("dependencies", SymKind::Section, None, false),
            sym("serde", SymKind::Section, None, false),
        ],
        rejected_syms: &["version"],
        ..Default::default()
    });
}
