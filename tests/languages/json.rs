use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("name", SymKind::Section, None, false),
            sym("scripts", SymKind::Section, None, false),
            sym("build", SymKind::Section, None, false),
        ],
        rejected_syms: &["deep_leaf"],
        ..Default::default()
    });
}
