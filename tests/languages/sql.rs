use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("accounts", SymKind::Class, None, false),
            sym("active_accounts", SymKind::Class, None, false),
        ],
        ..Default::default()
    });
}
