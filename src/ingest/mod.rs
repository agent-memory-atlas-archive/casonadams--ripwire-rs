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
        assert!(classify_capture("other").is_none());
        for tail in [
            "function",
            "cjsexport",
            "method",
            "protomethod",
            "class",
            "object",
            "struct",
            "interface",
            "var",
            "constant",
            "attribute",
            "property",
            "macro",
            "module",
            "section",
            "yamlkey",
            "field",
            "x",
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
            "macro",
            "type",
            "value",
            "through",
            "call",
            "y",
        ] {
            assert!(classify_capture(&format!("reference.{tail}")).is_some());
        }
    }

    #[test]
    fn test_all_31_grammars() {
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
}
