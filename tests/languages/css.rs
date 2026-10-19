use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym(".panel", SymKind::Class, None, false),
            sym("fadeIn", SymKind::Function, None, false),
        ],
        ..Default::default()
    });
}
