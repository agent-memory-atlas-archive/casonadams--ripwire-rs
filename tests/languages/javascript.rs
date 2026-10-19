use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Interner, Lang, LanguageRegistry, RefRole, SymKind, ingest_in_memory_files};

#[test]
fn test_javascript_and_jsx_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "javascript",
        files: &[(
            "src/app.js",
            Lang::JavaScript,
            b"import './init.js';\nconst API_VERSION = 2;\nconst tempLocal = 0;\nclass BaseCtrl {}\nclass UserCtrl extends BaseCtrl {\n  process() {\n    JSON.stringify({});\n    validate();\n  }\n}\nfunction validate() {\n  return true;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "API_VERSION", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "UserCtrl", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "process", kind: SymKind::Method, scope: Some("UserCtrl"), has_body_span: true },
            ExpectedSym { name: "validate", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["tempLocal"],
        expected_edges: &[("process", "validate")],
        expect_external: true,
        expected_lego: Some(("BaseCtrl", "UserCtrl")),
        expected_import: Some("./init.js"),
    });

    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[(
            "src/View.jsx",
            Lang::JavaScript,
            b"module.exports.exportedFn = () => {};\nother.notExport = () => {};\nFoo.prototype.protoFn = function() {};\nexport function View() { return <span><Footer /></span>; }",
        )],
        &reg,
        &interner,
    );
    let syms: Vec<&str> = ingest
        .symbols
        .iter()
        .map(|s| interner.resolve(&s.name))
        .collect();
    assert!(syms.contains(&"exportedFn"));
    assert!(syms.contains(&"protoFn"));
    assert!(!syms.contains(&"notExport"));
    let calls: Vec<&str> = ingest
        .references
        .iter()
        .filter(|r| r.role == RefRole::Call)
        .map(|r| interner.resolve(&r.callee_name))
        .collect();
    assert!(calls.contains(&"Footer"));
    assert!(!calls.contains(&"span"));
}
