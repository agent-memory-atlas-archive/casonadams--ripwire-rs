pub mod astro;
pub mod extract;
pub mod markdown;
pub mod pipeline;
pub mod registry;
pub mod yaml_guard;

pub use extract::*;
pub use pipeline::*;
pub use registry::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Lang;

    #[test]
    fn test_tier1_registry_and_parsing() {
        let reg = LanguageRegistry::new_tier1().unwrap();
        assert!(reg.get(Lang::Rust).is_some() && reg.get(Lang::Python).is_some());

        let (lang, query) = reg.get(Lang::Rust).unwrap();
        let res = parse_file_content(
            b"fn run() { log(); } fn log() {}",
            lang,
            query,
            Lang::Rust,
            "src/lib.rs",
        );
        assert_eq!(res.defs.len(), 2);
        assert_eq!(res.refs.len(), 1);
        assert_eq!(res.refs[0].callee_name, "log");
    }

    #[test]
    fn test_classify_captures() {
        assert!(matches!(classify_capture("name"), Some(Capture::Name)));
        assert!(matches!(
            classify_capture("import.path"),
            Some(Capture::ImportPath)
        ));
        assert!(classify_capture("other").is_none());
        assert!(classify_capture("definition.unknown_tag").is_none());
        assert!(classify_capture("reference.unknown_tag").is_none());
        for tail in [
            "function",
            "cjsexport",
            "method",
            "protomethod",
            "class",
            "object",
            "specialization",
            "struct",
            "interface",
            "type",
            "var",
            "constant",
            "attribute",
            "property",
            "enummember",
            "macro",
            "testmacroblock",
            "module",
            "section",
            "yamlkey",
            "field",
        ] {
            assert!(classify_capture(&format!("definition.{tail}")).is_some());
        }
        for tail in [
            "read",
            "attribute",
            "write",
            "import",
            "extends",
            "implementation",
            "class",
            "macro",
            "type",
            "value",
            "through",
            "call",
            "bare",
            "operator",
        ] {
            assert!(classify_capture(&format!("reference.{tail}")).is_some());
        }
    }

    #[test]
    fn test_all_scm_capture_vocabulary_and_patterns_valid() {
        let reg = LanguageRegistry::new().unwrap();
        let all_langs = [
            Lang::Rust,
            Lang::TypeScript,
            Lang::JavaScript,
            Lang::Python,
            Lang::Go,
            Lang::C,
            Lang::Cpp,
            Lang::Cuda,
            Lang::Metal,
            Lang::Java,
            Lang::CSharp,
            Lang::Swift,
            Lang::Bash,
            Lang::Ruby,
            Lang::Php,
            Lang::Lua,
            Lang::Elixir,
            Lang::Dart,
            Lang::Kotlin,
            Lang::GDScript,
            Lang::Json,
            Lang::Toml,
            Lang::Yaml,
            Lang::Markdown,
            Lang::Zig,
            Lang::Solidity,
            Lang::Hcl,
            Lang::Scala,
            Lang::Erlang,
            Lang::Html,
            Lang::Css,
            Lang::Sql,
            Lang::Proto,
            Lang::Graphql,
            Lang::Dockerfile,
            Lang::ObjC,
        ];
        for lang in all_langs {
            let (_, query) = reg.get(lang).expect("registered language");
            for cap_name in query.capture_names() {
                if *cap_name == "elixir.context" {
                    continue;
                }
                assert!(
                    classify_capture(cap_name).is_some(),
                    "Lang {:?} tags.scm contains unrecognized capture '@{}'",
                    lang,
                    cap_name
                );
            }
        }
    }

    #[test]
    fn test_oop_inheritance_lego_and_uses_contract() {
        let reg = LanguageRegistry::new().unwrap();
        let oop_fixtures: &[(&str, Lang, &str, &str, &str)] = &[
            (
                "shapes.dart",
                Lang::Dart,
                "abstract class Shape {}\nfinal class Circle extends Shape {}\nclass Square implements Shape {}\nclass Polygon extends Base with Shape {}",
                "Shape",
                "Circle",
            ),
            (
                "service.ts",
                Lang::TypeScript,
                "export interface Service {}\nexport class Worker implements Service {}\nexport class Child extends Worker {}",
                "Service",
                "Worker",
            ),
            (
                "app.js",
                Lang::JavaScript,
                "class Base {}\nclass Derived extends Base {}",
                "Base",
                "Derived",
            ),
            (
                "model.py",
                Lang::Python,
                "class BaseModel:\n    pass\nclass User(BaseModel):\n    pass",
                "BaseModel",
                "User",
            ),
            (
                "Worker.java",
                Lang::Java,
                "public interface Service {}\npublic class Worker implements Service {}",
                "Service",
                "Worker",
            ),
            (
                "Handler.kt",
                Lang::Kotlin,
                "interface Handler\nclass HttpHandler : Handler {}",
                "Handler",
                "HttpHandler",
            ),
            (
                "Repo.cs",
                Lang::CSharp,
                "public interface IRepo {}\npublic class SqlRepo : IRepo {}",
                "IRepo",
                "SqlRepo",
            ),
            (
                "Greeter.swift",
                Lang::Swift,
                "protocol Greeter {}\nstruct ConsoleGreeter: Greeter {}",
                "Greeter",
                "ConsoleGreeter",
            ),
            (
                "lib.rs",
                Lang::Rust,
                "pub trait Runner {}\npub struct FastRunner;\nimpl Runner for FastRunner {}",
                "Runner",
                "FastRunner",
            ),
            (
                "engine.cpp",
                Lang::Cpp,
                "class Base {};\nclass Derived : public Base {};",
                "Base",
                "Derived",
            ),
            (
                "Service.php",
                Lang::Php,
                "<?php\ninterface Contract {}\nclass Concrete implements Contract {}",
                "Contract",
                "Concrete",
            ),
            (
                "job.rb",
                Lang::Ruby,
                "class BaseJob\nend\nclass MailJob < BaseJob\nend",
                "BaseJob",
                "MailJob",
            ),
            (
                "App.scala",
                Lang::Scala,
                "trait Runner\nclass TaskRunner extends Runner",
                "Runner",
                "TaskRunner",
            ),
            (
                "Token.sol",
                Lang::Solidity,
                "interface IERC20 {}\ncontract MyToken is IERC20 {}",
                "IERC20",
                "MyToken",
            ),
            (
                "Widget.m",
                Lang::ObjC,
                "@interface Base\n@end\n@interface Sub : Base\n@end",
                "Base",
                "Sub",
            ),
            (
                "enemy.gd",
                Lang::GDScript,
                "class Actor:\n  pass\nclass Boss:\n  extends Actor",
                "Actor",
                "Boss",
            ),
        ];
        for &(path, lang, code, base_name, impl_name) in oop_fixtures {
            let interner = crate::domain::Interner::new();
            let ingest = ingest_in_memory_files(&[(path, lang, code.as_bytes())], &reg, &interner);
            let has_extends = ingest.references.iter().any(|r| {
                r.role == crate::domain::RefRole::Extends
                    && interner.resolve(&r.callee_name) == base_name
            });
            assert!(
                has_extends,
                "Lang {:?} ({}) failed to extract RefRole::Extends for '{}'",
                lang, path, base_name
            );
            let lego = crate::mcp::retrieval::call_lego(&ingest, &interner, base_name);
            let impls = lego["implementors"].as_array().expect("implementors array");
            assert!(
                impls.iter().any(|i| i["name"] == impl_name),
                "Lang {:?} ({}) call_lego('{}') did not find implementor '{}', got {:?}",
                lang,
                path,
                base_name,
                impl_name,
                impls
            );
        }
    }

    #[test]
    fn test_all_36_grammars() {
        let reg = LanguageRegistry::new().unwrap();
        let fixtures: Vec<(&str, Lang, &str, usize, usize)> = vec![
            ("src/main.rs", Lang::Rust, "fn main() { run(); }", 1, 1),
            (
                "src/index.ts",
                Lang::TypeScript,
                "function greet(name: string) { log(name); }",
                1,
                1,
            ),
            (
                "src/app.js",
                Lang::JavaScript,
                "function add(a, b) { return sum(a, b); }",
                1,
                1,
            ),
            (
                "src/worker.py",
                Lang::Python,
                "def process(x):\n    handle(x)",
                1,
                1,
            ),
            (
                "src/server.go",
                Lang::Go,
                "package main\nfunc Worker() { doWork() }",
                1,
                1,
            ),
            ("src/util.c", Lang::C, "void init(void) { setup(); }", 1, 1),
            (
                "src/engine.cpp",
                Lang::Cpp,
                "void start() { boot(); }",
                1,
                1,
            ),
            (
                "src/kernel.cu",
                Lang::Cuda,
                "__global__ void kernel() { launch(); }",
                1,
                1,
            ),
            (
                "src/shader.metal",
                Lang::Metal,
                "void compute_shader() { do_compute(); }",
                1,
                1,
            ),
            (
                "src/App.java",
                Lang::Java,
                "class App { void main() { start(); } }",
                2,
                1,
            ),
            (
                "src/Service.cs",
                Lang::CSharp,
                "class Service { void Run() { Execute(); } }",
                2,
                1,
            ),
            (
                "src/entry.swift",
                Lang::Swift,
                "func launch() { begin() }",
                1,
                1,
            ),
            ("src/script.sh", Lang::Bash, "run() { echo hi; }", 1, 0),
            (
                "src/item.rb",
                Lang::Ruby,
                "class Item\n  def show\n    render()\n  end\nend",
                2,
                1,
            ),
            (
                "src/ctrl.php",
                Lang::Php,
                "<?php\nclass Controller {\n  public function handle() {\n    execute();\n  }\n}",
                2,
                1,
            ),
            (
                "src/alg.lua",
                Lang::Lua,
                "function solve(x)\n  compute(x)\nend",
                1,
                1,
            ),
            (
                "src/calc.ex",
                Lang::Elixir,
                "defmodule Calc do\n  def add(a, b) do\n    sum(a, b)\n  end\nend",
                2,
                1,
            ),
            (
                "src/box.dart",
                Lang::Dart,
                "class Box {\n  int compute(int x) {\n    return transform(x);\n  }\n}",
                2,
                1,
            ),
            (
                "src/handler.kt",
                Lang::Kotlin,
                "class Handler {\n  fun process(id: Int, flag: Boolean) {\n    execute(id)\n  }\n}",
                2,
                1,
            ),
            (
                "src/hero.gd",
                Lang::GDScript,
                "class_name Hero\nfunc move(dir):\n  step(dir)",
                2,
                1,
            ),
            (
                "package.json",
                Lang::Json,
                "{\n  \"name\": \"ripwire\",\n  \"scripts\": { \"test\": \"cargo test\" }\n}",
                2,
                0,
            ),
            (
                "Cargo.toml",
                Lang::Toml,
                "[package]\nname = \"ripwire\"\nversion = \"0.1.0\"",
                2,
                0,
            ),
            (
                "config.yaml",
                Lang::Yaml,
                "name: pipeline\nstages:\n  build: true",
                2,
                0,
            ),
            (
                "README.md",
                Lang::Markdown,
                "# Project\nOverview\n## Architecture\nDetails",
                3,
                0,
            ),
            (
                "src/main.zig",
                Lang::Zig,
                "pub fn add(a: i32, b: i32) i32 { return a + b; }\npub fn run() void { _ = add(1, 2); }",
                2,
                1,
            ),
            (
                "contracts/Token.sol",
                Lang::Solidity,
                "contract Token { function transfer() public { doTransfer(); } }",
                2,
                1,
            ),
            (
                "infra/main.tf",
                Lang::Hcl,
                "resource \"aws_s3_bucket\" \"b\" { bucket = \"my-bucket\" }",
                1,
                0,
            ),
            (
                "src/Main.scala",
                Lang::Scala,
                "object Main { def run(): Unit = { execute() } }",
                2,
                1,
            ),
            (
                "src/app.erl",
                Lang::Erlang,
                "-module(app).\nstart() -> run().",
                1,
                1,
            ),
            (
                "index.html",
                Lang::Html,
                "<html><body><div id=\"app\"></div></body></html>",
                1,
                0,
            ),
            (
                "style.css",
                Lang::Css,
                ".card { display: flex; }\n@keyframes spin { from { transform: rotate(0deg); } }",
                2,
                0,
            ),
            (
                "schema.sql",
                Lang::Sql,
                "CREATE TABLE users ( id INT );",
                1,
                0,
            ),
            (
                "api.proto",
                Lang::Proto,
                "syntax = \"proto3\"; service Svc { rpc Call() returns (); } message Msg {}",
                2,
                0,
            ),
            (
                "schema.graphql",
                Lang::Graphql,
                "type User { id: ID! }",
                1,
                0,
            ),
            (
                "Dockerfile",
                Lang::Dockerfile,
                "FROM alpine:latest AS builder\nRUN apk add git",
                1,
                0,
            ),
            (
                "src/widget.m",
                Lang::ObjC,
                "#import \"widget.h\"\n@interface Widget\n- (void)paint;\n@end\n@implementation Widget\n- (void)paint { [self draw]; }\n@end",
                3,
                1,
            ),
        ];

        for (path, lang, code, min_defs, min_refs) in fixtures {
            let (ts_lang, query) = reg.get(lang).unwrap();
            let res = parse_file_content(code.as_bytes(), ts_lang, query, lang, path);
            assert!(
                res.defs.len() >= min_defs,
                "Language {:?} at {} expected at least {} defs, got {}",
                lang,
                path,
                min_defs,
                res.defs.len()
            );
            assert!(
                res.refs.len() >= min_refs,
                "Language {:?} at {} expected at least {} refs, got {}",
                lang,
                path,
                min_refs,
                res.refs.len()
            );
        }
    }

    #[test]
    fn test_specialized_language_handlers() {
        let reg = LanguageRegistry::new().unwrap();

        // Astro frontmatter extraction
        let astro_code =
            b"---\nimport Nav from './Nav.astro';\nfunction setup() { run(); }\n---\n<Nav />";
        let (ts_lang, query) = reg.get(Lang::TypeScript).unwrap();
        let astro_res = parse_file_content(
            astro_code,
            ts_lang,
            query,
            Lang::TypeScript,
            "src/Component.astro",
        );
        assert!(
            !astro_res.defs.is_empty(),
            "Astro should extract frontmatter defs"
        );
        assert_eq!(astro_res.defs[0].name, "setup");
        assert!(
            !astro_res.refs.is_empty(),
            "Astro should extract frontmatter refs"
        );
        assert_eq!(astro_res.refs[0].callee_name, "run");

        // Dart sibling function_body span extension
        let dart_code =
            b"class Calculator {\n  int add(int a, int b) {\n    return helper(a, b);\n  }\n}";
        let (dart_lang, dart_query) = reg.get(Lang::Dart).unwrap();
        let dart_res =
            parse_file_content(dart_code, dart_lang, dart_query, Lang::Dart, "calc.dart");
        let add_def = dart_res
            .defs
            .iter()
            .find(|d| d.name == "add")
            .expect("found add");
        assert!(
            add_def.end_byte > add_def.start_byte + 25,
            "Dart def span must extend through body"
        );

        // Elixir arity-qualified naming
        let elixir_code = b"defmodule M do\n  def sum(a, b) do\n    a + b\n  end\nend";
        let (ex_lang, ex_query) = reg.get(Lang::Elixir).unwrap();
        let ex_res = parse_file_content(elixir_code, ex_lang, ex_query, Lang::Elixir, "m.ex");
        assert!(
            ex_res.defs.iter().any(|d| d.name == "sum/2"),
            "Elixir function must be arity-qualified: sum/2"
        );

        // Kotlin positional scope and parameter count
        let kt_code =
            b"class Service {\n  fun handle(id: Int, flag: Boolean) {\n    run(id)\n  }\n}";
        let (kt_lang, kt_query) = reg.get(Lang::Kotlin).unwrap();
        let kt_res = parse_file_content(kt_code, kt_lang, kt_query, Lang::Kotlin, "Service.kt");
        let fn_def = kt_res
            .defs
            .iter()
            .find(|d| d.name == "handle")
            .expect("found handle");
        assert_eq!(fn_def.scope.as_deref(), Some("Service"));
        assert_eq!(fn_def.params, 2);
    }
    #[test]
    fn test_dart_end_to_end_parity_contracts() {
        let reg = LanguageRegistry::new().unwrap();
        let interner = crate::domain::Interner::new();
        let shapes_code = br#"
sealed class TaskOutcome {}
final class TaskCompleted extends TaskOutcome {}
final class TaskSkipped extends TaskOutcome {}
final class TaskMissing extends TaskOutcome {}
final class TaskAmbiguous extends TaskOutcome {}
final class TaskConflicted extends TaskOutcome {}
"#;
        let ctrl_code = br#"
class ApiController {
  void executeAction() {
    recordAudit();
  }
}
void recordAudit() {}
"#;
        let svc_code = br#"
import 'package:core/core.dart';

String normalizeInput(String raw) {
  return raw;
}

void dispatchBatch(String item) {
  final normalized = normalizeInput(item);
  sendPayload(normalized);
}

void sendPayload(String payload) {}

class WorkerService {
  Future<void> executeAction(String taskId, String mode) async {
    if (taskId.isEmpty) {
      return;
    }
    dispatchBatch(taskId);
  }
}
"#;
        let html_code = b"<html><body><div id=\"app\"><span>text</span></div></body></html>";
        let ingest = ingest_in_memory_files(
            &[
                ("src/domain/outcomes.dart", Lang::Dart, shapes_code),
                ("src/api/controller.dart", Lang::Dart, ctrl_code),
                ("src/service/worker.dart", Lang::Dart, svc_code),
                ("src/views/template.html", Lang::Html, html_code),
            ],
            &reg,
            &interner,
        );
        let graph = crate::graph::Graph::build(&ingest, &interner);
        let pr = graph.page_rank(&crate::rank::PageRankConfig::default());

        let lego = crate::mcp::retrieval::call_lego(&ingest, &interner, "TaskOutcome");
        assert_eq!(lego["count"], 5);
        let mut lego_xml = Vec::new();
        crate::serialize::serialize_lego_xml("lib", &lego, &mut lego_xml).unwrap();
        let lego_str = String::from_utf8(lego_xml).unwrap();
        assert!(lego_str.contains("implementors=\"5\""));
        assert!(lego_str.contains("p=\"src/domain/outcomes.dart:"));

        let uses = crate::mcp::graph_tools::call_uses(&ingest, &interner, "TaskOutcome");
        assert_eq!(uses["count"], 5);

        let path_val = crate::mcp::graph_tools::call_path_between(
            &ingest,
            &graph,
            &interner,
            "executeAction",
            "normalizeInput",
        );
        assert_eq!(path_val["reachable"], true);
        assert_eq!(path_val["hops"], 2);
        assert_eq!(path_val["from_defs"], 2);
        assert_eq!(path_val["to_defs"], 1);

        let has_div_cls = ingest
            .symbols
            .iter()
            .any(|s| s.kind == crate::domain::SymKind::Class && interner.resolve(&s.name) == "div");
        assert!(!has_div_cls);

        let slice_xml = crate::slice::execute_slice(
            &ingest,
            &reg,
            &interner,
            "dispatchBatch:normalized",
            crate::slice::SliceFlowDir::Fwd,
            8,
            Some(svc_code),
        )
        .unwrap();
        assert!(slice_xml.contains("v=\"normalized\""));
        assert!(!slice_xml.contains("steps=\"0\""));

        let req_sym = ingest
            .symbols
            .iter()
            .find(|s| {
                interner.resolve(&s.name) == "executeAction"
                    && s.scope.map(|sc| interner.resolve(&sc)) == Some("WorkerService")
            })
            .expect("WorkerService::executeAction");
        assert!(req_sym.sig_end_byte > req_sym.sig_start_byte);
        assert!(req_sym.sig_end_byte < req_sym.end_byte);
        let sig_slice = &svc_code[req_sym.sig_start_byte as usize..req_sym.sig_end_byte as usize];
        let sig_str = String::from_utf8_lossy(sig_slice);
        assert!(sig_str.contains("Future<void> executeAction(String taskId, String mode)"));

        let for_val = crate::mcp::retrieval::call_for_task_with_options(
            &ingest,
            &interner,
            &pr.ranks,
            "Worker executeAction",
            crate::mcp::retrieval::ForTaskOptions {
                root: ".",
                graph: Some(&graph),
                ..Default::default()
            },
        );
        assert!(for_val["count"].as_u64().unwrap_or(0) > 0);

        let mut conn_buf = Vec::new();
        crate::cli::execute_connect_query(
            "executeAction,normalizeInput",
            &ingest,
            &graph,
            &interner,
            &mut conn_buf,
        )
        .unwrap();
        assert!(
            String::from_utf8(conn_buf)
                .unwrap()
                .contains("<edge from=\"executeAction\" to=\"normalizeInput\"/>")
        );

        let mut sd_buf = Vec::new();
        crate::cli::execute_safe_delete_query(
            ".",
            "normalizeInput",
            &ingest,
            &graph,
            &interner,
            &mut sd_buf,
        )
        .unwrap();
        assert!(
            String::from_utf8(sd_buf)
                .unwrap()
                .contains("p=\"src/service/worker.dart:")
        );

        let mut around_buf = Vec::new();
        crate::cli::execute_around_query(
            ".",
            "dispatchBatch",
            crate::cli::AroundConfig {
                depth: 1,
                fanout: 32,
            },
            &ingest,
            &graph,
            &interner,
            &mut around_buf,
        )
        .unwrap();
        let around_str = String::from_utf8(around_buf).unwrap();
        assert!(around_str.contains("n=\"executeAction\""));
        assert!(around_str.contains("n=\"normalizeInput\""));

        let sub_sym = ingest
            .symbols
            .iter()
            .find(|s| interner.resolve(&s.name) == "dispatchBatch")
            .unwrap();
        let head_contract = crate::gate::contract::SignatureContract {
            name: "dispatchBatch".to_string(),
            params: sub_sym.params,
            is_public: crate::gate::contract::is_symbol_name_public("dispatchBatch", Lang::Dart),
        };
        let ec_rep = crate::gate::edit_check::check_symbol_contract(
            sub_sym.id,
            Some(&head_contract),
            &ingest,
            &graph,
            &interner,
        )
        .unwrap();
        assert_eq!(
            ec_rep.status,
            crate::gate::contract::ContractStatus::Unchanged
        );
    }

    #[test]
    fn test_embedded_sql_table_lineage_across_languages() {
        let reg = LanguageRegistry::new().unwrap();
        let interner = crate::domain::Interner::new();

        let sql_migration = br#"
CREATE TABLE mobile.user_records (
    id UUID PRIMARY KEY,
    status TEXT NOT NULL
);
CREATE TABLE audit_events (
    id SERIAL PRIMARY KEY
);
"#;
        let dart_repo = br#"
class AccountRepository {
  AccountRepository(this.a, {required this.b});
  Future<void> createRecord(dynamic db) async {
    await db.query("INSERT INTO mobile.user_records (id, status) VALUES ($1, $2)");
  }
  Future<void> loadActive(dynamic db) async {
    await db.query("SELECT r.id FROM \"mobile\".\"user_records\" r JOIN audit_events a ON a.id = r.id WHERE r.status = 'active'");
  }
  Future<void> purgeOld(dynamic db, {int limit = 10, bool force = false}) async {
    await db.query("DELETE FROM ONLY mobile.user_records WHERE status = 'archived'");
    await db.query("UPDATE audit_events SET id = 0");
  }
  Future<void> runMaintenance(dynamic db) async {
    await purgeOld(db, limit: 50, force: true);
  }
}
"#;
        let caller_service = br#"
class MobileSwapService {
  void executeSwap() {
    AccountRepository(1, b: 2);
  }
}
"#;
        let ingest = ingest_in_memory_files(
            &[
                ("migrations/001_init.sql", Lang::Sql, sql_migration),
                ("lib/db/account_repo.dart", Lang::Dart, dart_repo),
                ("lib/mobile/swap_service.dart", Lang::Dart, caller_service),
            ],
            &reg,
            &interner,
        );
        let graph = crate::graph::Graph::build(&ingest, &interner);
        let ranks = vec![1.0; ingest.symbols.len()];

        let uses_val = crate::mcp::graph_tools::call_uses(&ingest, &interner, "user_records");
        assert_eq!(uses_val["count"], 3);
        let sites = uses_val["uses"].as_array().unwrap();
        assert!(
            sites
                .iter()
                .any(|s| s["role"] == "write" && s["enclosing"] == "createRecord")
        );
        assert!(
            sites
                .iter()
                .any(|s| s["role"] == "read" && s["enclosing"] == "loadActive")
        );
        assert!(
            sites
                .iter()
                .any(|s| s["role"] == "write" && s["enclosing"] == "purgeOld")
        );

        let table_sym = ingest
            .symbols
            .iter()
            .find(|s| interner.resolve(&s.name) == "user_records")
            .expect("SQL table user_records");
        let nav = crate::navigate::navigate_1hop(table_sym.id, &ingest, &graph, &ranks, &interner)
            .unwrap();
        assert_eq!(nav.callers.len(), 3);

        let purge_sym = ingest
            .symbols
            .iter()
            .find(|s| interner.resolve(&s.name) == "purgeOld")
            .expect("purgeOld method");
        assert_eq!(purge_sym.params, 3);
        let purge_nav =
            crate::navigate::navigate_1hop(purge_sym.id, &ingest, &graph, &ranks, &interner)
                .unwrap();
        assert_eq!(purge_nav.callers.len(), 1);
        assert_eq!(purge_nav.callers[0].name, "runMaintenance");

        let mut callers_buf = Vec::new();
        crate::cli::execute_nav_symbol_query(
            "AccountRepository",
            crate::cli::NavMode::Callers,
            &ingest,
            &graph,
            &ranks,
            &interner,
            &mut callers_buf,
        )
        .unwrap();
        assert!(String::from_utf8_lossy(&callers_buf).contains("n=\"executeSwap\""));
    }
}
