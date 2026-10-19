use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("FAST", SymKind::Var, Some("Mode"), false),
            sym("TaskWorker", SymKind::Class, None, true),
            sym("execute", SymKind::Function, Some("TaskWorker"), true),
            sym("job_id", SymKind::Field, Some("TaskWorker"), false),
            sym("run_step", SymKind::Function, None, true),
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
