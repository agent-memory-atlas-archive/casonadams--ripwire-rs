use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
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
            ExpectedSym { name: "start", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "step", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("start", "step")],
        expect_external: true,
        expected_lego: None,
        expected_import: None,
    });
}
