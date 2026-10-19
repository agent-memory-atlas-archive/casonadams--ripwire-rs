use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("Pipeline", SymKind::Section, None, true),
            sym("custom_attr", SymKind::Var, Some("Pipeline"), false),
            sym("state", SymKind::Var, Some("Pipeline"), false),
            sym("execute/1", SymKind::Function, Some("Pipeline"), true),
            sym("transform/1", SymKind::Function, Some("Pipeline"), true),
            sym("\"runs pipeline\"/0", SymKind::Function, Some("Pipeline"), true),
        ],
        rejected_syms: &["doc", "x/0"],
        expected_edges: &[("execute/1", "transform/1")],
        expect_external: true,
        ..Default::default()
    });
    assert!(Lang::Elixir.semantics().is_std_or_builtin_member("map/2"));
}
