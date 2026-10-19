use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_proto_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "proto",
        files: &[(
            "api.proto",
            Lang::Proto,
            b"syntax = \"proto3\";\nservice AuthService {\n  rpc Login (LoginReq) returns (LoginResp);\n}\nmessage LoginReq {\n  string user = 1;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "AuthService", kind: SymKind::Class, scope: None, has_body_span: false },
            ExpectedSym { name: "Login", kind: SymKind::Function, scope: None, has_body_span: false },
            ExpectedSym { name: "LoginReq", kind: SymKind::Class, scope: None, has_body_span: false },
        ],
        rejected_syms: &[],
        expected_edges: &[],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    });
}
