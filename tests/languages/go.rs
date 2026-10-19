use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
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
            ExpectedSym { name: "Token", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "Server", kind: SymKind::Struct, scope: None, has_body_span: false },
            ExpectedSym { name: "Serve", kind: SymKind::Method, scope: Some("Server"), has_body_span: true },
            ExpectedSym { name: "handleConn", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("Serve", "handleConn")],
        expect_external: true,
        expected_lego: None,
        expected_import: Some("fmt"),
    });
    assert!(Lang::Go.semantics().is_symbol_name_public("Serve"));
    assert!(!Lang::Go.semantics().is_symbol_name_public("handleConn"));
}
