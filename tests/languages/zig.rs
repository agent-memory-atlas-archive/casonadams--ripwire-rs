use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("Engine", SymKind::Class, None, false),
            sym("Mode", SymKind::Var, None, false),
            sym("Payload", SymKind::Class, None, false),
            sym("start", SymKind::Function, None, true),
            sym("helper", SymKind::Function, None, true),
        ],
        expected_edges: &[("start", "helper")],
        expect_external: true,
        ..Default::default()
    });
}
