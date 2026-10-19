use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::mcp::retrieval::call_lego;
use ripwire_rs::{Interner, Lang, LanguageRegistry, SymKind, ingest_in_memory_files};

#[test]
fn test_cpp_graph_protocol_and_macro_gates() {
    verify_protocol_case(&GraphProtocolCase {
        label: "cpp",
        files: &[(
            "src/engine.cpp",
            Lang::Cpp,
            b"#include \"engine.h\"\n#define NOOP_MACRO(x)\nstatic const int MAX_THREADS = 8;\nconstexpr int kValidConst = 7;\nint mutable_global = 0;\nint uninitialized_global;\nclass BaseEngine {\npublic:\n    static constexpr int kClassConst = 9;\n    static int s_mutable;\n    int count;\n};\ntemplate<> class BaseEngine<int> {};\nclass RenderEngine : public BaseEngine {\npublic:\n    void render() {\n        std::lock_guard<std::mutex> guard(m);\n        std::move(0);\n        flushBuffers();\n    }\n};\nvoid flushBuffers() {\n    return;\n}\nTEST_CASE(\"suite\") {\n    flushBuffers();\n}\n",
        )],
        expected_syms: &[
            ExpectedSym { name: "MAX_THREADS", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "kValidConst", kind: SymKind::Var, scope: None, has_body_span: false },
            ExpectedSym { name: "kClassConst", kind: SymKind::Var, scope: Some("BaseEngine"), has_body_span: false },
            ExpectedSym { name: "count", kind: SymKind::Field, scope: Some("BaseEngine"), has_body_span: false },
            ExpectedSym { name: "RenderEngine", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "render", kind: SymKind::Method, scope: Some("RenderEngine"), has_body_span: true },
            ExpectedSym { name: "flushBuffers", kind: SymKind::Function, scope: None, has_body_span: true },
            ExpectedSym { name: "TEST_CASE", kind: SymKind::Macro, scope: None, has_body_span: true },
        ],
        rejected_syms: &["NOOP_MACRO", "mutable_global", "uninitialized_global", "s_mutable", "guard"],
        expected_edges: &[("render", "flushBuffers")],
        expect_external: true,
        expected_lego: Some(("BaseEngine", "RenderEngine")),
        expected_import: Some("engine.h"),
    });
}

#[test]
fn test_cpp_macro_blanking_and_attribute_macros() {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let src = br#"
#define CALL_HELPER(x) do { real_helper(x); if (x) {} } while(0)
class Widget {
    REGISTER_WIDGET(Widget)
    [[noreturn]] RW_COLD_NOINLINE
    static void handlePanic(const char* file, int line) noexcept;
public:
    void work() {
        int v = static_cast<int>(1.0);
        CALL_HELPER(v);
    }
};
RW_COLD_NOINLINE
void Widget::handlePanic(const char* file, int line) noexcept {}
void real_helper(int x) {}
"#;
    let ingest = ingest_in_memory_files(&[("widget.cpp", Lang::Cpp, src)], &registry, &interner);
    let names: Vec<&str> = ingest.symbols.iter().map(|s| &interner[s.name]).collect();
    assert!(names.contains(&"Widget"));
    assert!(names.contains(&"work"));
    assert!(names.contains(&"handlePanic"));
    assert!(names.contains(&"CALL_HELPER"));
    let callees: Vec<&str> = ingest
        .references
        .iter()
        .map(|r| &interner[r.callee_name])
        .collect();
    assert!(callees.contains(&"real_helper"));
    assert!(!callees.contains(&"static_cast"));
    assert!(!callees.contains(&"if"));

    let lego = call_lego(&ingest, &interner, "Widget");
    let methods = lego["methods"].as_array().unwrap();
    let method_names: Vec<&str> = methods.iter().filter_map(|m| m["name"].as_str()).collect();
    assert!(method_names.contains(&"handlePanic"));
}
