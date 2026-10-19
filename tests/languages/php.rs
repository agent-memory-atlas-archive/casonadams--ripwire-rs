use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("Guard", SymKind::Interface, None, true),
            sym("TokenGuard", SymKind::Class, None, true),
            sym("check", SymKind::Method, Some("TokenGuard"), true),
            sym("verifyToken", SymKind::Function, None, true),
        ],
        expected_edges: &[("check", "verifyToken")],
        expected_lego: Some(("Guard", "TokenGuard")),
        expected_import: Some("Guard"),
        ..Default::default()
    });
    assert!(Lang::Php.semantics().is_std_or_builtin_member("explode"));
}
