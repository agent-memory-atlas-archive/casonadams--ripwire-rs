use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("AuthService", SymKind::Class, None, false),
            sym("Login", SymKind::Function, None, false),
            sym("LoginReq", SymKind::Class, None, false),
        ],
        ..Default::default()
    });
}
