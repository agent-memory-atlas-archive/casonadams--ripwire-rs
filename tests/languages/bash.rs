use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
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
            ExpectedSym { name: "run_deploy", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "build_image", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["LOCAL_VAR"],
        expected_edges: &[("run_deploy", "build_image")],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    });
    assert!(!Lang::Bash.semantics().supports_slice());
}
