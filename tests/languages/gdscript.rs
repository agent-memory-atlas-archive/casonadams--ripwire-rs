use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("Actor", SymKind::Class, None, true),
            sym("Boss", SymKind::Class, None, true),
            sym("tick", SymKind::Method, Some("Boss"), true),
            sym("advance", SymKind::Method, Some("Boss"), true),
        ],
        expected_edges: &[("tick", "advance")],
        expected_lego: Some(("Actor", "Boss")),
        ..Default::default()
    });
    assert!(
        Lang::GDScript
            .semantics()
            .is_std_or_builtin_member("queue_free")
    );
}
