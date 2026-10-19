use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
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
            ExpectedSym {
                name: "Node",
                kind: SymKind::Interface,
                scope: None,
                has_body_span: false,
            },
            ExpectedSym {
                name: "Account",
                kind: SymKind::Class,
                scope: None,
                has_body_span: false,
            },
            ExpectedSym {
                name: "id",
                kind: SymKind::Field,
                scope: None,
                has_body_span: false,
            },
        ],
        rejected_syms: &[],
        expected_edges: &[],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    });
}
