use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};
use std::collections::HashMap;

#[test]
fn test_dart_graph_protocol_and_imports() {
    verify_protocol_case(&GraphProtocolCase {
        label: "dart",
        files: &[(
            "lib/service.dart",
            Lang::Dart,
            b"import 'package:core/core.dart';\nabstract class Runner {}\nclass TaskRunner extends Runner {\n  static List<String> allCols() => [];\n  void execute() {\n    print('skip');\n    Future.wait([]);\n    performStep();\n  }\n  void performStep() {\n    return;\n  }\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Runner", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "TaskRunner", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "allCols", kind: SymKind::Method, scope: Some("TaskRunner"), has_body_span: true },
            ExpectedSym { name: "execute", kind: SymKind::Method, scope: Some("TaskRunner"), has_body_span: true },
            ExpectedSym { name: "performStep", kind: SymKind::Method, scope: Some("TaskRunner"), has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("execute", "performStep")],
        expect_external: true,
        expected_lego: Some(("Runner", "TaskRunner")),
        expected_import: Some("package:core/core.dart"),
    });

    let mut dart_idx = HashMap::new();
    dart_idx.insert("packages/core/lib/src/logger.dart", 10);
    dart_idx.insert("packages/api/lib/src/db.dart", 11);
    let sem = Lang::Dart.semantics();
    assert_eq!(
        sem.resolve_import_target(
            "packages/api/lib/src/service.dart",
            "'package:core/src/logger.dart'",
            &dart_idx
        ),
        Some(10)
    );
    assert_eq!(
        sem.resolve_import_target(
            "packages/api/lib/src/service.dart",
            "'./db.dart'",
            &dart_idx
        ),
        Some(11)
    );
    assert_eq!(
        sem.resolve_import_target(
            "packages/api/lib/src/service.dart",
            "'dart:convert'",
            &dart_idx
        ),
        None
    );
}
