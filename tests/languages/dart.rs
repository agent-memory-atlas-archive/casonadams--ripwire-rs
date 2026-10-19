use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("Runner", SymKind::Class, None, true),
            sym("TaskRunner", SymKind::Class, None, true),
            sym("allCols", SymKind::Method, Some("TaskRunner"), true),
            sym("execute", SymKind::Method, Some("TaskRunner"), true),
            sym("performStep", SymKind::Method, Some("TaskRunner"), true),
        ],
        expected_edges: &[("execute", "performStep")],
        expect_external: true,
        expected_lego: Some(("Runner", "TaskRunner")),
        expected_import: Some("package:core/core.dart"),
        ..Default::default()
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
