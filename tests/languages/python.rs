use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};
use std::collections::HashMap;

#[test]
fn test_python_graph_protocol_and_gates() {
    verify_protocol_case(&GraphProtocolCase {
        label: "python",
        files: &[(
            "pkg/worker.py",
            Lang::Python,
            b"import os\nclass Mode(Enum):\n    FAST = 1\nclass BaseWorker:\n    pass\nclass TaskWorker(BaseWorker):\n    untyped_noise = 0\n    def execute(self, other):\n        self.job_id = 7\n        other.temp_val = 20\n        os.getcwd()\n        run_step()\ndef run_step():\n    return 1\nself.top_level = 30\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "FAST", kind: SymKind::Var, scope: Some("Mode"), has_body_span: false },
            ExpectedSym { name: "TaskWorker", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "execute", kind: SymKind::Function, scope: Some("TaskWorker"), has_body_span: true },
            ExpectedSym { name: "job_id", kind: SymKind::Field, scope: Some("TaskWorker"), has_body_span: false },
            ExpectedSym { name: "run_step", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["untyped_noise", "temp_val", "top_level"],
        expected_edges: &[("execute", "run_step")],
        expect_external: true,
        expected_lego: Some(("BaseWorker", "TaskWorker")),
        expected_import: Some("os"),
    });

    let mut file_index = HashMap::new();
    file_index.insert("pkg/mod_a.py", 6);
    let sem = Lang::Python.semantics();
    assert_eq!(
        sem.resolve_import_target("pkg/sub.py", ".mod_a", &file_index),
        Some(6)
    );
    assert!(!sem.is_entrypoint_symbol("main", None, SymKind::Function, "docs/build_docs.py"));
}
