use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_cuda_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "cuda",
        files: &[(
            "src/kernel.cu",
            Lang::Cuda,
            b"#include \"kernel.cuh\"\n__constant__ float d_weights[16];\nvoid launch(void) {\n    run_kernel();\n}\n__global__ void run_kernel(void) {\n    return;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "d_weights", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "launch", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "run_kernel", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("launch", "run_kernel")],
        expect_external: false,
        expected_lego: None,
        expected_import: Some("kernel.cuh"),
    });
}
