use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_java_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "java",
        files: &[(
            "src/Worker.java",
            Lang::Java,
            b"import java.util.List;\npublic interface Job { void runJob(); }\npublic class Worker implements Job {\n    public static final int MAX_Q = 10;\n    private int instanceField = 1;\n    public void runJob() {\n        Collections.emptyList();\n        helperStep();\n    }\n    private void helperStep() {\n        return;\n    }\n}\n",
        )],
        expected_syms: &[
            sym("Job", SymKind::Interface, None, true),
            sym("Worker", SymKind::Class, None, true),
            sym("MAX_Q", SymKind::Var, Some("Worker"), false),
            sym("runJob", SymKind::Method, Some("Worker"), true),
            sym("helperStep", SymKind::Method, Some("Worker"), true),
        ],
        rejected_syms: &["instanceField"],
        expected_edges: &[("runJob", "helperStep")],
        expect_external: true,
        expected_lego: Some(("Job", "Worker")),
        expected_import: Some("List"),
    });
    assert!(Lang::Java.semantics().is_entrypoint_symbol(
        "main",
        Some("Worker"),
        SymKind::Method,
        "src/Worker.java"
    ));
}
