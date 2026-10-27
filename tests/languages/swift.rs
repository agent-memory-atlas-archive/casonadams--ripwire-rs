use super::harness::{GraphProtocolCase, owner_qualified_callees, sym, verify_protocol_case};
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

#[test]
fn swift_member_calls_never_bind_to_a_same_named_local() {
    let source = "struct Info {\n    func summary() -> String { \"\" }\n}\n\
struct Runner {\n    let name: String = \"\"\n    func run(info: Info) {\n        let summary = info.summary()\n        print(summary)\n    }\n}\n";
    let callees = |caller| {
        owner_qualified_callees(
            &[("src/Runner.swift", Lang::Swift, source.as_bytes())],
            caller,
        )
    };
    assert_eq!(callees("run"), ["Info.summary"]);
}
