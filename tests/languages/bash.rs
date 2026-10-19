use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_bash_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "bash",
        files: &[(
            "scripts/deploy.sh",
            Lang::Bash,
            b"#!/usr/bin/env bash\nrun_deploy() {\n  LOCAL_VAR=1\n  build_image\n}\nbuild_image() {\n  echo ok\n}\n",
        )],
        expected_syms: &[
            sym("run_deploy", SymKind::Function, None, true),
            sym("build_image", SymKind::Function, None, true),
        ],
        rejected_syms: &["LOCAL_VAR"],
        expected_edges: &[("run_deploy", "build_image")],
        ..Default::default()
    });
    assert!(!Lang::Bash.semantics().supports_slice());
}
