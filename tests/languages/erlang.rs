use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_erlang_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "erlang",
        files: &[(
            "src/server.erl",
            Lang::Erlang,
            b"-module(server).\nstart() ->\n    lists:map(fun step/1, []),\n    step(1).\nstep(X) ->\n    X.\n",
        )],
        expected_syms: &[
            sym("start", SymKind::Function, None, true),
            sym("step", SymKind::Function, None, true),
        ],
        expected_edges: &[("start", "step")],
        expect_external: true,
        ..Default::default()
    });
}
