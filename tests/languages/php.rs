use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_php_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "php",
        files: &[(
            "src/Auth.php",
            Lang::Php,
            b"<?php\nuse App\\Contracts\\Guard;\ninterface Guard {}\nclass TokenGuard implements Guard {\n    public function check() {\n        verifyToken();\n    }\n}\nfunction verifyToken() {\n    return true;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Guard", kind: SymKind::Interface, scope: None, has_body_span: true },
            ExpectedSym { name: "TokenGuard", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "check", kind: SymKind::Method, scope: Some("TokenGuard"), has_body_span: true },
            ExpectedSym { name: "verifyToken", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("check", "verifyToken")],
        expect_external: false,
        expected_lego: Some(("Guard", "TokenGuard")),
        expected_import: Some("Guard"),
    });
    assert!(Lang::Php.semantics().is_std_or_builtin_member("explode"));
}
