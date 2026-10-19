use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_zig_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "zig",
        files: &[(
            "src/main.zig",
            Lang::Zig,
            b"const Engine = struct {\n    pub fn start() void {\n        std.print();\n        helper();\n    }\n};\nconst Mode = enum { fast, slow };\nconst Payload = union { raw: u32 };\npub fn helper() void {\n    return;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Engine", kind: SymKind::Class, scope: None, has_body_span: false },
            ExpectedSym { name: "Mode", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "Payload", kind: SymKind::Class, scope: None, has_body_span: false },
            ExpectedSym { name: "start", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "helper", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("start", "helper")],
        expect_external: true,
        expected_lego: None,
        expected_import: None,
    });
}
