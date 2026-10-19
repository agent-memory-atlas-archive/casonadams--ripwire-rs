use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_swift_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "swift",
        files: &[(
            "src/Runner.swift",
            Lang::Swift,
            b"protocol Greeter {\n    func greet()\n}\nstruct ConsoleGreeter: Greeter {\n    func greet() {\n        DispatchQueue.main()\n        logMsg()\n    }\n    func logMsg() {\n        return\n    }\n}\n",
        )],
        expected_syms: &[
            sym("Greeter", SymKind::Interface, None, true),
            sym("ConsoleGreeter", SymKind::Class, None, true),
            sym("greet", SymKind::Function, Some("ConsoleGreeter"), true),
            sym("logMsg", SymKind::Function, Some("ConsoleGreeter"), true),
        ],
        expected_edges: &[("greet", "logMsg")],
        expect_external: true,
        expected_lego: Some(("Greeter", "ConsoleGreeter")),
        ..Default::default()
    });
    assert!(Lang::Swift.semantics().is_std_or_builtin_member("append"));
}
