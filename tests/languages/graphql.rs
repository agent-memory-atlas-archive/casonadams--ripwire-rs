use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_graphql_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "graphql",
        files: &[(
            "schema.graphql",
            Lang::Graphql,
            b"interface Node {\n  id: ID!\n}\ntype Account {\n  id: ID!\n}\n",
        )],
        expected_syms: &[
            sym("Node", SymKind::Interface, None, false),
            sym("Account", SymKind::Class, None, false),
            sym("id", SymKind::Field, None, false),
        ],
        ..Default::default()
    });
}
