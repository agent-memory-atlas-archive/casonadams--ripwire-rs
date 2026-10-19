use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Interner, Lang, LanguageRegistry, RefRole, SymKind, ingest_in_memory_files};
use std::collections::HashMap;

#[test]
fn test_typescript_and_tsx_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "typescript",
        files: &[(
            "src/service.ts",
            Lang::TypeScript,
            b"import { log } from './logger';\nexport const MAX_BATCH = 64;\nexport const camelNoise = 1;\nexport interface Handler { handle(): void; }\nexport class ApiHandler implements Handler {\n  handle(): void {\n    Array.isArray([]);\n    dispatch();\n  }\n}\nexport function dispatch(): void {\n  return;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "MAX_BATCH", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "Handler", kind: SymKind::Interface, scope: None, has_body_span: true },
            ExpectedSym { name: "ApiHandler", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "handle", kind: SymKind::Method, scope: Some("ApiHandler"), has_body_span: true },
            ExpectedSym { name: "dispatch", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["camelNoise"],
        expected_edges: &[("handle", "dispatch")],
        expect_external: true,
        expected_lego: Some(("Handler", "ApiHandler")),
        expected_import: Some("./logger"),
    });

    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[(
            "src/App.tsx",
            Lang::TypeScript,
            b"export function App() { return <div><Header /><UI.Button>ok</UI.Button></div>; }",
        )],
        &reg,
        &interner,
    );
    let calls: Vec<&str> = ingest
        .references
        .iter()
        .filter(|r| r.role == RefRole::Call)
        .map(|r| interner.resolve(&r.callee_name))
        .collect();
    assert!(calls.contains(&"Header"));
    assert!(calls.contains(&"Button"));
    assert!(!calls.contains(&"div"));

    let mut file_index = HashMap::new();
    file_index.insert("packages/utils/src/index.ts", 1);
    file_index.insert("packages/utils/src/logger.ts", 2);
    file_index.insert("packages/agent/src/helper.ts", 3);
    let sem = Lang::TypeScript.semantics();
    assert_eq!(
        sem.resolve_import_target(
            "packages/agent/src/main.ts",
            "'@scope/pkg-utils'",
            &file_index
        ),
        Some(1)
    );
    assert_eq!(
        sem.resolve_import_target(
            "packages/agent/src/main.ts",
            "'@scope/utils/logger'",
            &file_index
        ),
        Some(2)
    );
    assert_eq!(
        sem.resolve_import_target("packages/agent/src/main.ts", "'./helper.js'", &file_index),
        Some(3)
    );
    assert_eq!(
        sem.resolve_import_target("packages/agent/src/main.ts", "'external-pkg'", &file_index),
        None
    );
}
