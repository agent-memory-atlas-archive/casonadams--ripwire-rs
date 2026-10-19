use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_elixir_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "elixir",
        files: &[(
            "lib/pipeline.ex",
            Lang::Elixir,
            b"defmodule Pipeline do\n  @doc \"hidden\"\n  @custom_attr 42\n  @type state :: map()\n  def execute(x) do\n    Enum.map(x, &(&1))\n    transform(x)\n  end\n  defp transform(x) do\n    x\n  end\n  test \"runs pipeline\" do\n    execute(1)\n  end\nend\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Pipeline", kind: SymKind::Section, scope: None, has_body_span: true },
            ExpectedSym { name: "custom_attr", kind: SymKind::Var, scope: Some("Pipeline"), has_body_span: false },
            ExpectedSym { name: "state", kind: SymKind::Var, scope: Some("Pipeline"), has_body_span: false },
            ExpectedSym { name: "execute/1", kind: SymKind::Function, scope: Some("Pipeline"), has_body_span: true },
            ExpectedSym { name: "transform/1", kind: SymKind::Function, scope: Some("Pipeline"), has_body_span: true },
            ExpectedSym { name: "\"runs pipeline\"/0", kind: SymKind::Function, scope: Some("Pipeline"), has_body_span: true },
        ],
        rejected_syms: &["doc", "x/0"],
        expected_edges: &[("execute/1", "transform/1")],
        expect_external: true,
        expected_lego: None,
        expected_import: None,
    });
    assert!(Lang::Elixir.semantics().is_std_or_builtin_member("map/2"));
}
