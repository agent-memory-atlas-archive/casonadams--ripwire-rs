use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
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
    });
}
