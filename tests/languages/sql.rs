use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_sql_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "sql",
        files: &[(
            "schema.sql",
            Lang::Sql,
            b"CREATE TABLE accounts ( id INT );\nCREATE VIEW active_accounts AS SELECT id FROM accounts;\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "accounts", kind: SymKind::Class, scope: None, has_body_span: false },
            ExpectedSym { name: "active_accounts", kind: SymKind::Class, scope: None, has_body_span: false },
        ],
        rejected_syms: &[],
        expected_edges: &[],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    });
}
