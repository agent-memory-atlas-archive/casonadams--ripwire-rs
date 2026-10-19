use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("BaseService", SymKind::Interface, None, true),
            sym("MainService", SymKind::Class, None, true),
            sym("run", SymKind::Function, Some("MainService"), true),
            sym("step", SymKind::Function, Some("MainService"), true),
        ],
        expected_edges: &[("run", "step")],
        expect_external: true,
        expected_lego: Some(("BaseService", "MainService")),
        ..Default::default()
    });
}
