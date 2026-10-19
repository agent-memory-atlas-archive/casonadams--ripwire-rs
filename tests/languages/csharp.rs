use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_csharp_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "csharp",
        files: &[(
            "src/Repo.cs",
            Lang::CSharp,
            b"using System.IO;\npublic interface IStore {}\npublic class SqlStore : IStore {\n    public const int LIMIT = 50;\n    private int retryCounter = 0;\n    public void Save() {\n        Console.WriteLine();\n        Commit();\n    }\n    private void Commit() {\n        return;\n    }\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "IStore", kind: SymKind::Interface, scope: None, has_body_span: true },
            ExpectedSym { name: "SqlStore", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "LIMIT", kind: SymKind::Var, scope: Some("SqlStore"), has_body_span: false },
            ExpectedSym { name: "Save", kind: SymKind::Method, scope: Some("SqlStore"), has_body_span: true },
            ExpectedSym { name: "Commit", kind: SymKind::Method, scope: Some("SqlStore"), has_body_span: true },
        ],
        rejected_syms: &["retryCounter"],
        expected_edges: &[("Save", "Commit")],
        expect_external: true,
        expected_lego: Some(("IStore", "SqlStore")),
        expected_import: Some("IO"),
    });
    assert!(Lang::CSharp.semantics().is_entrypoint_symbol(
        "Main",
        Some("Program"),
        SymKind::Method,
        "src/Program.cs"
    ));
}
