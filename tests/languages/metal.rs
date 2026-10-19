use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_metal_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "metal",
        files: &[(
            "src/shader.metal",
            Lang::Metal,
            b"#include \"shader.h\"\nvoid compute_pass() {\n    metal::sin(0.0);\n    shade_vertex();\n}\nvoid shade_vertex() {\n    return;\n}\n",
        )],
        expected_syms: &[
            sym("compute_pass", SymKind::Function, None, true),
            sym("shade_vertex", SymKind::Function, None, true),
        ],
        expected_edges: &[("compute_pass", "shade_vertex")],
        expect_external: true,
        expected_import: Some("shader.h"),
        ..Default::default()
    });
}
