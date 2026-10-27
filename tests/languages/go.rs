use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_go_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "go",
        files: &[(
            "pkg/server.go",
            Lang::Go,
            b"package pkg\nimport \"fmt\"\ntype Token = string\ntype Server struct{}\nfunc (s *Server) Serve() {\n\tfmt.Println(\"up\")\n\thandleConn()\n}\nfunc handleConn() {\n\treturn\n}\n",
        )],
        expected_syms: &[
            sym("Token", SymKind::Var, None, false),
            sym("Server", SymKind::Struct, None, false),
            sym("Serve", SymKind::Method, Some("Server"), true),
            sym("handleConn", SymKind::Function, None, true),
        ],
        expected_edges: &[("Serve", "handleConn")],
        expect_external: true,
        expected_import: Some("fmt"),
        ..Default::default()
    });
    assert!(Lang::Go.semantics().is_symbol_name_public("Serve"));
    assert!(!Lang::Go.semantics().is_symbol_name_public("handleConn"));
}

#[test]
fn test_go_grouped_parameters_count_each_name_for_arity() {
    verify_protocol_case(&GraphProtocolCase {
        label: "go-grouped-params",
        files: &[(
            "check.go",
            Lang::Go,
            b"package main\nfunc checkContains(t *T, got, expected string) {}\nfunc probe() {\n\tcheckContains(nil, \"a\", \"b\")\n}\n",
        )],
        expected_syms: &[
            sym("checkContains", SymKind::Function, None, false),
            sym("probe", SymKind::Function, None, false),
        ],
        expected_edges: &[("probe", "checkContains")],
        ..Default::default()
    });
}
