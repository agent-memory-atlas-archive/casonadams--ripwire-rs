use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
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
            ExpectedSym { name: "Job", kind: SymKind::Interface, scope: None, has_body_span: true },
            ExpectedSym { name: "Worker", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "MAX_Q", kind: SymKind::Var, scope: Some("Worker"), has_body_span: false },
            ExpectedSym { name: "runJob", kind: SymKind::Method, scope: Some("Worker"), has_body_span: true },
            ExpectedSym { name: "helperStep", kind: SymKind::Method, scope: Some("Worker"), has_body_span: true },
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
