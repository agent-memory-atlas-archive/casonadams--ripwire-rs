use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_kotlin_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "kotlin",
        files: &[(
            "src/Handler.kt",
            Lang::Kotlin,
            b"import com.example.Util\ninterface Handler\nclass HttpHandler : Handler {\n    fun handle() {\n        Collections.emptyList()\n        dispatchReq()\n    }\n    fun dispatchReq() {\n        return\n    }\n}\n",
        )],
        expected_syms: &[
            sym("Handler", SymKind::Class, None, false),
            sym("HttpHandler", SymKind::Class, None, true),
            sym("handle", SymKind::Function, Some("HttpHandler"), true),
            sym("dispatchReq", SymKind::Function, Some("HttpHandler"), true),
        ],
        expected_edges: &[("handle", "dispatchReq")],
        expect_external: true,
        expected_lego: Some(("Handler", "HttpHandler")),
        expected_import: Some("Util"),
        ..Default::default()
    });
}
