use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
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
            ExpectedSym { name: "compute_pass", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "shade_vertex", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("compute_pass", "shade_vertex")],
        expect_external: true,
        expected_lego: None,
        expected_import: Some("shader.h"),
    });
}
