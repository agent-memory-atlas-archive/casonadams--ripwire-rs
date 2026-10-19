use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_gdscript_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "gdscript",
        files: &[(
            "scripts/boss.gd",
            Lang::GDScript,
            b"class Actor:\n\tpass\nclass Boss:\n\textends Actor\n\tfunc tick():\n\t\tadvance()\n\tfunc advance():\n\t\tpass\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Actor", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "Boss", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "tick", kind: SymKind::Method, scope: Some("Boss"), has_body_span: true },
            ExpectedSym { name: "advance", kind: SymKind::Method, scope: Some("Boss"), has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("tick", "advance")],
        expect_external: false,
        expected_lego: Some(("Actor", "Boss")),
        expected_import: None,
    });
    assert!(
        Lang::GDScript
            .semantics()
            .is_std_or_builtin_member("queue_free")
    );
}
