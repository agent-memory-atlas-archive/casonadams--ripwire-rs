use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_scala_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "scala",
        files: &[(
            "src/Service.scala",
            Lang::Scala,
            b"trait BaseService {\n  def run(): Unit\n}\nobject MainService extends BaseService {\n  def run(): Unit = {\n    List.empty()\n    step()\n  }\n  def step(): Unit = {\n    ()\n  }\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "BaseService", kind: SymKind::Interface, scope: None, has_body_span: true },
            ExpectedSym { name: "MainService", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "run", kind: SymKind::Function, scope: Some("MainService"), has_body_span: true },
            ExpectedSym { name: "step", kind: SymKind::Function, scope: Some("MainService"), has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("run", "step")],
        expect_external: true,
        expected_lego: Some(("BaseService", "MainService")),
        expected_import: None,
    });
}
