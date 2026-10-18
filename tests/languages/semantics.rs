use ripwire_rs::{
    Interner, Lang, LanguageRegistry, RefRole, SymKind, ingest_in_memory_files,
    resolve_all_references,
};
use std::collections::HashMap;

#[test]
fn test_language_semantics_import_resolution_and_properties() {
    let mut file_index = HashMap::new();
    file_index.insert("packages/utils/src/index.ts", 1);
    file_index.insert("packages/utils/src/logger.ts", 2);
    file_index.insert("packages/agent/src/helper.ts", 3);
    file_index.insert("crates/core/src/sub/worker.rs", 4);
    file_index.insert("crates/core/src/common.rs", 5);
    file_index.insert("pkg/mod_a.py", 6);
    file_index.insert("include/api.h", 7);

    let ts_sem = Lang::TypeScript.semantics();
    assert_eq!(
        ts_sem.resolve_import_target(
            "packages/agent/src/index.ts",
            "'@acme/acme-utils'",
            &file_index
        ),
        Some(1)
    );
    assert_eq!(
        ts_sem.resolve_import_target(
            "packages/agent/src/index.ts",
            "\"@acme/utils/logger.js\"",
            &file_index
        ),
        Some(2)
    );
    assert_eq!(
        ts_sem.resolve_import_target(
            "packages/agent/src/sub/run.ts",
            "'../helper.js'",
            &file_index
        ),
        Some(3)
    );
    assert_eq!(
        ts_sem.resolve_import_target("packages/agent/src/index.ts", "'react'", &file_index),
        None
    );

    let rs_sem = Lang::Rust.semantics();
    assert_eq!(
        rs_sem.resolve_import_target(
            "crates/core/src/lib.rs",
            "crate::sub::worker::run",
            &file_index
        ),
        Some(4)
    );
    assert_eq!(
        rs_sem.resolve_import_target(
            "crates/core/src/sub/mod.rs",
            "super::common:: Helper",
            &file_index
        ),
        Some(5)
    );
    assert_eq!(
        rs_sem.resolve_import_target("crates/core/src/sub/mod.rs", "self::worker", &file_index),
        Some(4)
    );
    assert_eq!(
        rs_sem.resolve_import_target("crates/core/src/lib.rs", "std::io", &file_index),
        None
    );

    let py_sem = Lang::Python.semantics();
    assert_eq!(
        py_sem.resolve_import_target("pkg/main.py", ".mod_a", &file_index),
        Some(6)
    );

    let c_sem = Lang::C.semantics();
    assert_eq!(
        c_sem.resolve_import_target("src/main.c", "\"../include/api.h\"", &file_index),
        Some(7)
    );
    assert_eq!(
        c_sem.resolve_import_target("src/main.c", "<stdio.h>", &file_index),
        None
    );
    assert!(ts_sem.is_std_or_builtin_qualifier("Array"));
    assert!(ts_sem.is_std_or_builtin_member("has"));
    assert!(ts_sem.is_std_or_builtin_member("dirname"));
    assert!(rs_sem.is_std_or_builtin_qualifier("std"));
    assert!(Lang::Cpp.semantics().is_std_or_builtin_qualifier("std"));
    assert!(rs_sem.is_entrypoint_symbol("main", None, SymKind::Function, "src/main.rs"));
    assert!(!rs_sem.is_entrypoint_symbol("main", None, SymKind::Function, "examples/demo.rs"));
    assert!(!py_sem.is_entrypoint_symbol("main", None, SymKind::Function, "docs/build_docs.py"));
}

#[test]
fn test_all_language_std_members_and_qualifiers() {
    let cases = [
        (Lang::Python, "loads", "json"),
        (Lang::Ruby, "to_s", "JSON"),
        (Lang::Lua, "insert", "table"),
        (Lang::Java, "toList", "Collections"),
        (Lang::Kotlin, "toList", "Collections"),
        (Lang::Scala, "toList", "Collections"),
        (Lang::CSharp, "ToList", "Enumerable"),
        (Lang::Swift, "append", "DispatchQueue"),
        (Lang::Go, "Read", "fmt"),
        (Lang::Dart, "toList", "Future"),
        (Lang::Elixir, "map", "Enum"),
        (Lang::Erlang, "map", "lists"),
        (Lang::Zig, "deinit", "std"),
        (Lang::Solidity, "encode", "abi"),
        (Lang::Metal, "size", "metal"),
        (Lang::ObjC, "objectForKey", "NSDictionary"),
    ];
    for (lang, member, qual) in cases {
        let sem = lang.semantics();
        assert!(sem.is_std_or_builtin_member(member), "lang={lang:?}");
        assert!(sem.is_std_or_builtin_qualifier(qual), "lang={lang:?}");
    }
    assert!(
        Lang::GDScript
            .semantics()
            .is_std_or_builtin_member("queue_free")
    );
    assert!(
        Lang::Php
            .semantics()
            .is_std_or_builtin_member("json_encode")
    );
    assert!(Lang::Metal.semantics().is_builtin_keyword("static_cast"));
    assert!(Lang::ObjC.semantics().is_builtin_keyword("static_cast"));

    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let files = [
        (
            "pkg/a.go",
            Lang::Go,
            b"package pkg\nfunc Println(s string) {}\n" as &[u8],
        ),
        (
            "pkg/b.go",
            Lang::Go,
            b"package pkg\nimport \"fmt\"\nfunc run() { fmt.Println(\"hi\") }\n" as &[u8],
        ),
    ];
    let ingest = ingest_in_memory_files(&files, &reg, &interner);
    let res = resolve_all_references(&ingest, &interner);
    assert!(res.edge_accumulators.is_empty());

    let dart_files = [(
        "lib/db.dart",
        Lang::Dart,
        b"class Db extends common.BaseDb {\n  static List<String> allCols() => [];\n  void run() { Db.allCols(); print('x'); }\n}\n"
            as &[u8],
    )];
    let dart_ingest = ingest_in_memory_files(&dart_files, &reg, &interner);
    let all_cols_defs: Vec<_> = dart_ingest
        .symbols
        .iter()
        .filter(|s| interner.resolve(&s.name) == "allCols")
        .collect();
    assert_eq!(all_cols_defs.len(), 1);
    assert_eq!(all_cols_defs[0].kind, SymKind::Method);
    let extends_refs: Vec<_> = dart_ingest
        .references
        .iter()
        .filter(|r| r.role == RefRole::Extends)
        .map(|r| interner.resolve(&r.callee_name))
        .collect();
    assert_eq!(extends_refs, vec!["BaseDb"]);
    let mut dart_idx = HashMap::new();
    dart_idx.insert("packages/core/lib/src/logger.dart", 10);
    dart_idx.insert("packages/api/lib/src/db.dart", 11);
    let dart_sem = Lang::Dart.semantics();
    assert_eq!(
        dart_sem.resolve_import_target(
            "packages/api/lib/src/service.dart",
            "'package:core/src/logger.dart'",
            &dart_idx
        ),
        Some(10)
    );
    assert_eq!(
        dart_sem.resolve_import_target(
            "packages/api/lib/src/service.dart",
            "'./db.dart'",
            &dart_idx
        ),
        Some(11)
    );
    assert_eq!(
        dart_sem.resolve_import_target(
            "packages/api/lib/src/service.dart",
            "'dart:convert'",
            &dart_idx
        ),
        None
    );
}

#[test]
fn test_scm_capture_gates_filter_noise_and_keep_constants() {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();

    let files: &[(&str, Lang, &[u8])] = &[
        (
            "Config.java",
            Lang::Java,
            b"public class Config { public static final int MAX_POOL = 32; private int retryCount = 3; }",
        ),
        (
            "Settings.cs",
            Lang::CSharp,
            b"using System.IO;\npublic class Settings { public const int DEFAULT_PORT = 8080; private int timeoutMs = 100; }",
        ),
        (
            "constants.ts",
            Lang::TypeScript,
            b"export const MAX_RETRIES = 5;\nexport const retryBudget = 3;\nconst LOCAL_TEMP = 1;",
        ),
        (
            "constants.js",
            Lang::JavaScript,
            b"const API_TIMEOUT = 1000;\nconst localCache = {};",
        ),
        (
            "config.rb",
            Lang::Ruby,
            b"class AppConfig\n  MAX_WORKERS = 4\n  CamelAlias = String\nend",
        ),
        (
            "models.py",
            Lang::Python,
            b"class Status(Enum):\n    ACTIVE = 1\nclass User(BaseModel):\n    plain_attr = 2\n    def __init__(self, other):\n        self.user_id = 10\n        other.temp_val = 20\nself.top_level = 30\n",
        ),
        (
            "ci.yaml",
            Lang::Yaml,
            b"<<: *defaults\njobs:\n  build:\n    steps:\n      - name: test\n",
        ),
        (
            "worker.ex",
            Lang::Elixir,
            b"defmodule Worker do\n  @doc \"docs\"\n  @custom_attr 42\n  @type state :: map()\n  def run(item) do\n    process(item)\n  end\n  test \"runs worker\" do\n    run(1)\n  end\nend\n",
        ),
        (
            "macros.h",
            Lang::Cpp,
            b"#define EMPTY_GUARD(x)\n#define ADD_ONE(x) ((x) + 1)\n",
        ),
    ];

    let ingest = ingest_in_memory_files(files, &reg, &interner);
    let syms: Vec<&str> = ingest
        .symbols
        .iter()
        .map(|s| interner.resolve(&s.name))
        .collect();

    for kept in [
        "MAX_POOL",
        "DEFAULT_PORT",
        "MAX_RETRIES",
        "API_TIMEOUT",
        "MAX_WORKERS",
        "ACTIVE",
        "user_id",
        "jobs",
        "build",
        "Worker",
        "custom_attr",
        "state",
        "run/1",
        "\"runs worker\"/0",
        "ADD_ONE",
    ] {
        assert!(
            syms.contains(&kept),
            "expected kept symbol '{kept}', got {syms:?}"
        );
    }

    for dropped in [
        "retryCount",
        "timeoutMs",
        "retryBudget",
        "localCache",
        "CamelAlias",
        "plain_attr",
        "temp_val",
        "top_level",
        "<<",
        "steps",
        "name",
        "doc",
        "process/1",
        "item/0",
        "EMPTY_GUARD",
    ] {
        assert!(
            !syms.contains(&dropped),
            "expected noise symbol '{dropped}' to be filtered out, got {syms:?}"
        );
    }
}

#[test]
fn test_tsx_jsx_and_expanded_tier2_scm_patterns() {
    let reg = LanguageRegistry::new().unwrap();
    let interner = Interner::new();

    let files: &[(&str, Lang, &[u8])] = &[
        (
            "src/App.tsx",
            Lang::TypeScript,
            b"export function App() { return <div><Header /><UI.Button>ok</UI.Button></div>; }",
        ),
        (
            "src/View.jsx",
            Lang::JavaScript,
            b"export function View() { return <span><Footer /></span>; }",
        ),
        (
            "pkg/service.go",
            Lang::Go,
            b"package pkg\nimport \"fmt\"\ntype ID = string\ntype Service struct{}\nfunc (s *Service) Handle() { fmt.Println(\"ok\") }\n",
        ),
        (
            "src/Controller.php",
            Lang::Php,
            b"<?php\nuse App\\Services\\AuthService;\nuse Logger;\nclass Controller {}\n",
        ),
        (
            "src/engine.zig",
            Lang::Zig,
            b"const Engine = struct { pub fn tick() void {} };\nconst Mode = enum { fast, slow };\nconst Payload = union { raw: u32 };\npub fn main() void { Engine.tick(); }\n",
        ),
        (
            "src/Runner.scala",
            Lang::Scala,
            b"trait BaseRunner {\n  def contract(): Unit\n}\nobject Runner extends BaseRunner {\n  def contract(): Unit = { Helper.exec() }\n}\n",
        ),
        (
            "src/worker.erl",
            Lang::Erlang,
            b"-module(worker).\nrun() -> lists:map(fun handle/1, []), local_step().\n",
        ),
        (
            "infra/main.tf",
            Lang::Hcl,
            b"resource \"aws_s3_bucket\" \" logs \" {\n  bucket = \"my-logs\"\n}\n",
        ),
    ];

    let ingest = ingest_in_memory_files(files, &reg, &interner);
    let syms: Vec<&str> = ingest
        .symbols
        .iter()
        .map(|s| interner.resolve(&s.name))
        .collect();
    let call_refs: Vec<&str> = ingest
        .references
        .iter()
        .filter(|r| r.role == RefRole::Call)
        .map(|r| interner.resolve(&r.callee_name))
        .collect();
    let import_refs: Vec<&str> = ingest
        .references
        .iter()
        .filter(|r| r.role == RefRole::Import)
        .map(|r| interner.resolve(&r.callee_name))
        .collect();

    for expected_call in [
        "Header",
        "Button",
        "Footer",
        "tick",
        "exec",
        "map",
        "local_step",
    ] {
        assert!(
            call_refs.contains(&expected_call),
            "expected call ref '{expected_call}', got {call_refs:?}"
        );
    }
    assert!(!call_refs.contains(&"div"));
    assert!(!call_refs.contains(&"span"));

    for expected_imp in ["\"fmt\"", "AuthService", "Logger"] {
        assert!(
            import_refs.contains(&expected_imp),
            "expected import ref '{expected_imp}', got {import_refs:?}"
        );
    }

    for expected_sym in [
        "ID",
        "Engine",
        "Mode",
        "Payload",
        "contract",
        "aws_s3_bucket",
    ] {
        assert!(
            syms.contains(&expected_sym),
            "expected symbol '{expected_sym}', got {syms:?}"
        );
    }

    let go_handle = ingest
        .symbols
        .iter()
        .find(|s| interner.resolve(&s.name) == "Handle")
        .expect("Go Handle method");
    assert_eq!(
        go_handle.scope.map(|sc| interner.resolve(&sc)),
        Some("Service")
    );
}
