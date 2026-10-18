use ripwire_rs::mcp::retrieval::call_lego;
use ripwire_rs::{
    Graph, Interner, Lang, LanguageRegistry, NO_NODE, RefRole, SymKind, ingest_in_memory_files,
    resolve_all_references,
};

struct ExpectedSym {
    name: &'static str,
    kind: SymKind,
    scope: Option<&'static str>,
    has_body_span: bool,
}

struct GraphProtocolCase {
    label: &'static str,
    files: &'static [(&'static str, Lang, &'static [u8])],
    expected_syms: &'static [ExpectedSym],
    rejected_syms: &'static [&'static str],
    expected_edges: &'static [(&'static str, &'static str)],
    expect_external: bool,
    expected_lego: Option<(&'static str, &'static str)>,
    expected_import: Option<&'static str>,
}

fn assert_expected_sym(
    case_label: &str,
    exp: &ExpectedSym,
    ingest: &ripwire_rs::IngestResult,
    interner: &Interner,
) {
    let sym = ingest
        .symbols
        .iter()
        .filter(|s| {
            interner.resolve(&s.name) == exp.name
                && s.kind == exp.kind
                && s.scope.map(|sc| interner.resolve(&sc)) == exp.scope
        })
        .max_by_key(|s| u8::from(s.sig_end_byte < s.end_byte))
        .unwrap_or_else(|| {
            let actual: Vec<_> = ingest
                .symbols
                .iter()
                .map(|s| {
                    (
                        interner.resolve(&s.name),
                        s.kind,
                        s.scope.map(|sc| interner.resolve(&sc)),
                    )
                })
                .collect();
            panic!(
                "[{}] missing expected symbol ({}, {:?}, {:?}); actual={actual:?}",
                case_label, exp.name, exp.kind, exp.scope
            )
        });

    if exp.has_body_span {
        assert!(
            sym.sig_start_byte < sym.sig_end_byte && sym.sig_end_byte < sym.end_byte,
            "[{}] symbol '{}' failed span invariant: sig_start={} sig_end={} end={}",
            case_label,
            exp.name,
            sym.sig_start_byte,
            sym.sig_end_byte,
            sym.end_byte
        );
    }
}

fn assert_expected_edge(
    case_label: &str,
    caller_name: &str,
    callee_name: &str,
    ingest: &ripwire_rs::IngestResult,
    graph: &Graph,
    interner: &Interner,
) {
    let caller = ingest
        .symbols
        .iter()
        .filter(|s| interner.resolve(&s.name) == caller_name)
        .max_by_key(|s| u8::from(s.sig_end_byte < s.end_byte))
        .unwrap_or_else(|| panic!("[{case_label}] missing caller '{caller_name}'"));
    let callee = ingest
        .symbols
        .iter()
        .find(|s| interner.resolve(&s.name) == callee_name)
        .unwrap_or_else(|| panic!("[{case_label}] missing callee '{callee_name}'"));

    let attributed = ingest.references.iter().any(|r| {
        r.role == RefRole::Call
            && r.from_symbol != NO_NODE
            && r.from_symbol == caller.id
            && interner.resolve(&r.callee_name) == callee_name
    });
    assert!(
        attributed,
        "[{case_label}] call '{caller_name} -> {callee_name}' was not attributed to caller NodeId {}",
        caller.id
    );

    let has_csr_edge = graph
        .callees(caller.id)
        .any(|(to_id, _)| to_id == callee.id)
        && graph
            .callers(callee.id)
            .any(|(from_id, _)| from_id == caller.id);
    assert!(
        has_csr_edge,
        "[{case_label}] CSR graph missing edge '{caller_name} -> {callee_name}'"
    );
}

fn verify_protocol_case(reg: &LanguageRegistry, case: &GraphProtocolCase) {
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(case.files, reg, &interner);
    let resolution = resolve_all_references(&ingest, &interner);
    let graph = Graph::build(&ingest, &interner);

    for exp in case.expected_syms {
        assert_expected_sym(case.label, exp, &ingest, &interner);
    }

    for &rej in case.rejected_syms {
        let present = ingest
            .symbols
            .iter()
            .any(|s| interner.resolve(&s.name) == rej);
        assert!(
            !present,
            "[{}] noise symbol '{rej}' should be excluded",
            case.label
        );
    }

    for &(caller, callee) in case.expected_edges {
        assert_expected_edge(case.label, caller, callee, &ingest, &graph, &interner);
    }

    if case.expect_external {
        assert!(
            resolution.external >= 1,
            "[{}] expected >= 1 external stdlib call, got external={}",
            case.label,
            resolution.external
        );
    }

    if let Some((iface, impl_name)) = case.expected_lego {
        let lego = call_lego(&ingest, &interner, iface);
        let impls = lego["implementors"].as_array().expect("implementors array");
        assert!(
            impls.iter().any(|i| i["name"] == impl_name),
            "[{}] --lego({}) missing implementor '{}', got {impls:?}",
            case.label,
            iface,
            impl_name
        );
    }

    if let Some(imp_sub) = case.expected_import {
        let has_imp = ingest.references.iter().any(|r| {
            r.role == RefRole::Import && interner.resolve(&r.callee_name).contains(imp_sub)
        });
        assert!(
            has_imp,
            "[{}] missing RefRole::Import containing '{imp_sub}'",
            case.label
        );
    }
}

const PROTOCOL_CASES: &[GraphProtocolCase] = &[
    GraphProtocolCase {
        label: "rust",
        files: &[(
            "src/lib.rs",
            Lang::Rust,
            b"use crate::util::helper;\npub trait Runner { fn execute(&self); }\npub struct Engine;\nimpl Runner for Engine {\n    pub fn execute(&self) {\n        let local_noise = 1;\n        println!(\"{}\", local_noise);\n        std::mem::drop(local_noise);\n        step();\n    }\n}\npub fn step() {}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Runner", kind: SymKind::Interface, scope: None, has_body_span: true },
            ExpectedSym { name: "Engine", kind: SymKind::Class, scope: None, has_body_span: false },
            ExpectedSym { name: "execute", kind: SymKind::Method, scope: Some("Engine"), has_body_span: true },
            ExpectedSym { name: "step", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["local_noise"],
        expected_edges: &[("execute", "step")],
        expect_external: true,
        expected_lego: Some(("Runner", "Engine")),
        expected_import: Some("crate::util::helper"),
    },
    GraphProtocolCase {
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
    },
    GraphProtocolCase {
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
    },
    GraphProtocolCase {
        label: "python",
        files: &[(
            "pkg/worker.py",
            Lang::Python,
            b"import os\nclass Mode(Enum):\n    FAST = 1\nclass BaseWorker:\n    pass\nclass TaskWorker(BaseWorker):\n    untyped_noise = 0\n    def execute(self):\n        self.job_id = 7\n        os.getcwd()\n        run_step()\ndef run_step():\n    return 1\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "FAST", kind: SymKind::Var, scope: Some("Mode"), has_body_span: false },
            ExpectedSym { name: "TaskWorker", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "execute", kind: SymKind::Function, scope: Some("TaskWorker"), has_body_span: true },
            ExpectedSym { name: "job_id", kind: SymKind::Field, scope: Some("TaskWorker"), has_body_span: false },
            ExpectedSym { name: "run_step", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["untyped_noise"],
        expected_edges: &[("execute", "run_step")],
        expect_external: true,
        expected_lego: Some(("BaseWorker", "TaskWorker")),
        expected_import: Some("os"),
    },
    GraphProtocolCase {
        label: "go",
        files: &[(
            "pkg/server.go",
            Lang::Go,
            b"package pkg\nimport \"fmt\"\ntype Token = string\ntype Server struct{}\nfunc (s *Server) Serve() {\n\tfmt.Println(\"up\")\n\thandleConn()\n}\nfunc handleConn() {\n\treturn\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Token", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "Server", kind: SymKind::Struct, scope: None, has_body_span: false },
            ExpectedSym { name: "Serve", kind: SymKind::Method, scope: Some("Server"), has_body_span: true },
            ExpectedSym { name: "handleConn", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("Serve", "handleConn")],
        expect_external: true,
        expected_lego: None,
        expected_import: Some("fmt"),
    },
    GraphProtocolCase {
        label: "cpp",
        files: &[(
            "src/engine.cpp",
            Lang::Cpp,
            b"#include \"engine.h\"\n#define NOOP_MACRO(x)\nstatic const int MAX_THREADS = 8;\nint mutable_global = 0;\nclass BaseEngine {};\nclass RenderEngine : public BaseEngine {\npublic:\n    void render() {\n        std::move(0);\n        flushBuffers();\n    }\n};\nvoid flushBuffers() {\n    return;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "MAX_THREADS", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "RenderEngine", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "render", kind: SymKind::Method, scope: Some("RenderEngine"), has_body_span: true },
            ExpectedSym { name: "flushBuffers", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["NOOP_MACRO", "mutable_global"],
        expected_edges: &[("render", "flushBuffers")],
        expect_external: true,
        expected_lego: Some(("BaseEngine", "RenderEngine")),
        expected_import: Some("engine.h"),
    },
    GraphProtocolCase {
        label: "java",
        files: &[(
            "src/Worker.java",
            Lang::Java,
            b"import java.util.List;\npublic interface Job { void runJob(); }\npublic class Worker implements Job {\n    public static final int MAX_Q = 10;\n    private int instanceField = 1;\n    public void runJob() {\n        System.out.println();\n        helperStep();\n    }\n    private void helperStep() {\n        return;\n    }\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Job", kind: SymKind::Interface, scope: None, has_body_span: true },
            ExpectedSym { name: "Worker", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "MAX_Q", kind: SymKind::Var, scope: Some("Worker"), has_body_span: false },
            ExpectedSym { name: "runJob", kind: SymKind::Method, scope: Some("Worker"), has_body_span: true },
            ExpectedSym { name: "helperStep", kind: SymKind::Method, scope: Some("Worker"), has_body_span: true },
        ],
        rejected_syms: &["instanceField"],
        expected_edges: &[("runJob", "helperStep")],
        expect_external: false,
        expected_lego: Some(("Job", "Worker")),
        expected_import: Some("List"),
    },
    GraphProtocolCase {
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
    },
    GraphProtocolCase {
        label: "ruby",
        files: &[(
            "lib/job.rb",
            Lang::Ruby,
            b"class BaseJob\nend\nclass MailJob < BaseJob\n  MAX_RETRY = 3\n  AliasType = String\n  def perform\n    File.read('a')\n    deliver()\n  end\n  def deliver\n    true\n  end\nend\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "MailJob", kind: SymKind::Class, scope: None, has_body_span: false },
            ExpectedSym { name: "MAX_RETRY", kind: SymKind::Var, scope: Some("MailJob"), has_body_span: false },
            ExpectedSym { name: "perform", kind: SymKind::Method, scope: Some("MailJob"), has_body_span: true },
            ExpectedSym { name: "deliver", kind: SymKind::Method, scope: Some("MailJob"), has_body_span: true },
        ],
        rejected_syms: &["AliasType"],
        expected_edges: &[("perform", "deliver")],
        expect_external: true,
        expected_lego: Some(("BaseJob", "MailJob")),
        expected_import: None,
    },
    GraphProtocolCase {
        label: "elixir",
        files: &[(
            "lib/pipeline.ex",
            Lang::Elixir,
            b"defmodule Pipeline do\n  @doc \"hidden\"\n  def execute(x) do\n    Enum.map(x, &(&1))\n    transform(x)\n  end\n  defp transform(x) do\n    x\n  end\nend\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Pipeline", kind: SymKind::Section, scope: None, has_body_span: true },
            ExpectedSym { name: "execute/1", kind: SymKind::Function, scope: Some("Pipeline"), has_body_span: true },
            ExpectedSym { name: "transform/1", kind: SymKind::Function, scope: Some("Pipeline"), has_body_span: true },
        ],
        rejected_syms: &["doc", "x/0"],
        expected_edges: &[("execute/1", "transform/1")],
        expect_external: true,
        expected_lego: None,
        expected_import: None,
    },
    GraphProtocolCase {
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
    },
    GraphProtocolCase {
        label: "lua",
        files: &[(
            "src/mod.lua",
            Lang::Lua,
            b"local M = {}\nfunction M:run(x)\n  string.format('%s', x)\n  helper(x)\nend\nfunction helper(v)\n  return v\nend\nreturn M\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "run", kind: SymKind::Method, scope: Some("M"), has_body_span: true },
            ExpectedSym { name: "helper", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["M"],
        expected_edges: &[("run", "helper")],
        expect_external: true,
        expected_lego: None,
        expected_import: None,
    },
    GraphProtocolCase {
        label: "swift",
        files: &[(
            "src/Runner.swift",
            Lang::Swift,
            b"protocol Greeter {\n    func greet()\n}\nstruct ConsoleGreeter: Greeter {\n    func greet() {\n        DispatchQueue.main()\n        logMsg()\n    }\n    func logMsg() {\n        return\n    }\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Greeter", kind: SymKind::Interface, scope: None, has_body_span: true },
            ExpectedSym { name: "ConsoleGreeter", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "greet", kind: SymKind::Function, scope: Some("ConsoleGreeter"), has_body_span: true },
            ExpectedSym { name: "logMsg", kind: SymKind::Function, scope: Some("ConsoleGreeter"), has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("greet", "logMsg")],
        expect_external: true,
        expected_lego: Some(("Greeter", "ConsoleGreeter")),
        expected_import: None,
    },
    GraphProtocolCase {
        label: "kotlin",
        files: &[(
            "src/Handler.kt",
            Lang::Kotlin,
            b"import com.example.Util\ninterface Handler\nclass HttpHandler : Handler {\n    fun handle() {\n        Collections.emptyList()\n        dispatchReq()\n    }\n    fun dispatchReq() {\n        return\n    }\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Handler", kind: SymKind::Class, scope: None, has_body_span: false },
            ExpectedSym { name: "HttpHandler", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "handle", kind: SymKind::Function, scope: Some("HttpHandler"), has_body_span: true },
            ExpectedSym { name: "dispatchReq", kind: SymKind::Function, scope: Some("HttpHandler"), has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("handle", "dispatchReq")],
        expect_external: true,
        expected_lego: Some(("Handler", "HttpHandler")),
        expected_import: Some("Util"),
    },
    GraphProtocolCase {
        label: "scala",
        files: &[(
            "src/Service.scala",
            Lang::Scala,
            b"trait BaseService {\n  def run(): Unit\n}\nobject MainService extends BaseService {\n  def run(): Unit = {\n    List.empty()\n    step()\n  }\n  def step(): Unit = {\n    ()\n  }\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "BaseService", kind: SymKind::Interface, scope: None, has_body_span: true },
            ExpectedSym { name: "MainService", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "run", kind: SymKind::Function, scope: Some("MainService"), has_body_span: true },
            ExpectedSym { name: "step", kind: SymKind::Function, scope: Some("MainService"), has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("run", "step")],
        expect_external: true,
        expected_lego: Some(("BaseService", "MainService")),
        expected_import: None,
    },
    GraphProtocolCase {
        label: "dart",
        files: &[(
            "lib/service.dart",
            Lang::Dart,
            b"import 'package:core/core.dart';\nabstract class Runner {}\nclass TaskRunner extends Runner {\n  void execute() {\n    Future.wait([]);\n    performStep();\n  }\n  void performStep() {\n    return;\n  }\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Runner", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "TaskRunner", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "execute", kind: SymKind::Method, scope: Some("TaskRunner"), has_body_span: true },
            ExpectedSym { name: "performStep", kind: SymKind::Method, scope: Some("TaskRunner"), has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("execute", "performStep")],
        expect_external: true,
        expected_lego: Some(("Runner", "TaskRunner")),
        expected_import: Some("package:core/core.dart"),
    },
    GraphProtocolCase {
        label: "gdscript",
        files: &[(
            "scripts/boss.gd",
            Lang::GDScript,
            b"class Actor:\n\tpass\nclass Boss:\n\textends Actor\n\tfunc tick():\n\t\tadvance()\n\tfunc advance():\n\t\tpass\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Actor", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "Boss", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "tick", kind: SymKind::Method, scope: Some("Boss"), has_body_span: true },
            ExpectedSym { name: "advance", kind: SymKind::Method, scope: Some("Boss"), has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("tick", "advance")],
        expect_external: false,
        expected_lego: Some(("Actor", "Boss")),
        expected_import: None,
    },
    GraphProtocolCase {
        label: "bash",
        files: &[(
            "scripts/deploy.sh",
            Lang::Bash,
            b"#!/usr/bin/env bash\nrun_deploy() {\n  LOCAL_VAR=1\n  build_image\n}\nbuild_image() {\n  echo ok\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "run_deploy", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "build_image", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["LOCAL_VAR"],
        expected_edges: &[("run_deploy", "build_image")],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    },
    GraphProtocolCase {
        label: "zig",
        files: &[(
            "src/main.zig",
            Lang::Zig,
            b"const Engine = struct {\n    pub fn start() void {\n        std.print();\n        helper();\n    }\n};\npub fn helper() void {\n    return;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "Engine", kind: SymKind::Class, scope: None, has_body_span: false },
            ExpectedSym { name: "start", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "helper", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("start", "helper")],
        expect_external: true,
        expected_lego: None,
        expected_import: None,
    },
    GraphProtocolCase {
        label: "solidity",
        files: &[(
            "contracts/Vault.sol",
            Lang::Solidity,
            b"interface IVault {}\ncontract Vault is IVault {\n    function deposit() public {\n        abi.encode();\n        recordDeposit();\n    }\n    function recordDeposit() internal {\n        return;\n    }\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "IVault", kind: SymKind::Interface, scope: None, has_body_span: false },
            ExpectedSym { name: "Vault", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "deposit", kind: SymKind::Function, scope: Some("Vault"), has_body_span: true },
            ExpectedSym { name: "recordDeposit", kind: SymKind::Function, scope: Some("Vault"), has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("deposit", "recordDeposit")],
        expect_external: true,
        expected_lego: Some(("IVault", "Vault")),
        expected_import: None,
    },
    GraphProtocolCase {
        label: "erlang",
        files: &[(
            "src/server.erl",
            Lang::Erlang,
            b"-module(server).\nstart() ->\n    lists:map(fun step/1, []),\n    step(1).\nstep(X) ->\n    X.\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "start", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "step", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("start", "step")],
        expect_external: true,
        expected_lego: None,
        expected_import: None,
    },
    GraphProtocolCase {
        label: "objc",
        files: &[(
            "src/Renderer.m",
            Lang::ObjC,
            b"#import \"Renderer.h\"\n@interface BaseRenderer\n@end\n@interface MeshRenderer : BaseRenderer\n@end\n@implementation MeshRenderer\n- (void)draw {\n    initPipeline();\n}\n@end\nvoid initPipeline(void) {\n    return;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "MeshRenderer", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "draw", kind: SymKind::Method, scope: Some("MeshRenderer"), has_body_span: true },
            ExpectedSym { name: "initPipeline", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("draw", "initPipeline")],
        expect_external: false,
        expected_lego: Some(("BaseRenderer", "MeshRenderer")),
        expected_import: Some("Renderer.h"),
    },
    GraphProtocolCase {
        label: "c",
        files: &[(
            "src/core.c",
            Lang::C,
            b"#include \"core.h\"\n#define EMPTY_FN(x)\nstatic const int MAX_CONN = 16;\nint plain_global = 0;\nvoid start_server(void) {\n    accept_loop();\n}\nvoid accept_loop(void) {\n    return;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "MAX_CONN", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "start_server", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "accept_loop", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &["EMPTY_FN", "plain_global"],
        expected_edges: &[("start_server", "accept_loop")],
        expect_external: false,
        expected_lego: None,
        expected_import: Some("core.h"),
    },
    GraphProtocolCase {
        label: "cuda",
        files: &[(
            "src/kernel.cu",
            Lang::Cuda,
            b"#include \"kernel.cuh\"\n__constant__ float d_weights[16];\nvoid launch(void) {\n    run_kernel();\n}\n__global__ void run_kernel(void) {\n    return;\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "d_weights", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "launch", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "run_kernel", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("launch", "run_kernel")],
        expect_external: false,
        expected_lego: None,
        expected_import: Some("kernel.cuh"),
    },
];

#[test]
fn test_5_layer_language_graph_protocol_suite() {
    let reg = LanguageRegistry::new().unwrap();
    for case in PROTOCOL_CASES {
        verify_protocol_case(&reg, case);
    }
}
