use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("d_weights", SymKind::Var, None, false),
            sym("launch", SymKind::Function, None, true),
            sym("run_kernel", SymKind::Function, None, true),
        ],
        expected_edges: &[("launch", "run_kernel")],
        expected_import: Some("kernel.cuh"),
        ..Default::default()
    });
}
