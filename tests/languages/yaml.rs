use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("jobs", SymKind::Section, None, false),
            sym("test", SymKind::Section, None, false),
        ],
        rejected_syms: &["<<", "steps", "run"],
        ..Default::default()
    });
}
