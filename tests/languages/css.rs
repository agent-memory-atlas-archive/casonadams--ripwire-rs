use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_css_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "css",
        files: &[(
            "styles.css",
            Lang::Css,
            b".panel { display: flex; }\n@keyframes fadeIn { from { opacity: 0; } }\n",
        )],
        expected_syms: &[
            ExpectedSym {
                name: ".panel",
                kind: SymKind::Class,
                scope: None,
                has_body_span: false,
            },
            ExpectedSym {
                name: "fadeIn",
                kind: SymKind::Function,
                scope: None,
                has_body_span: false,
            },
        ],
        rejected_syms: &[],
        expected_edges: &[],
        expect_external: false,
        expected_lego: None,
        expected_import: None,
    });
}
