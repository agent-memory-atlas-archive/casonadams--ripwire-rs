use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
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
            ExpectedSym { name: "Greeter", kind: SymKind::Interface, scope: None, has_body_span: true },
            ExpectedSym { name: "ConsoleGreeter", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "greet", kind: SymKind::Function, scope: Some("ConsoleGreeter"), has_body_span: true },
            ExpectedSym { name: "logMsg", kind: SymKind::Function, scope: Some("ConsoleGreeter"), has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("greet", "logMsg")],
        expect_external: true,
        expected_lego: Some(("Greeter", "ConsoleGreeter")),
        expected_import: None,
    });
    assert!(Lang::Swift.semantics().is_std_or_builtin_member("append"));
}
