use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_brightscript_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "brightscript",
        files: &[
            (
                "source/main.bs",
                Lang::BrightScript,
                b"library \"pkg:/source/utils.brs\"\n\nsub main()\n    initApp()\n    m.runLoop()\nend sub\n\nfunction initApp() as object\n    helper()\n    return invalid\nend function\n",
            ),
            (
                "source/utils.brs",
                Lang::BrightScript,
                b"sub helper()\n    print \"helper\"\nend sub\n",
            ),
        ],
        expected_syms: &[
            sym("main", SymKind::Function, None, true),
            sym("initApp", SymKind::Function, None, true),
            sym("helper", SymKind::Function, None, true),
        ],
        expected_edges: &[
            ("main", "initApp"),
            ("initApp", "helper"),
        ],
        expected_import: Some("pkg:/source/utils.brs"),
        ..Default::default()
    });
    assert!(
        Lang::BrightScript
            .semantics()
            .is_std_or_builtin_member("createobject")
    );
    assert!(
        Lang::BrightScript
            .semantics()
            .is_std_or_builtin_qualifier("rosgnode")
    );
    assert_eq!(Lang::from_extension("brs"), Lang::BrightScript);
    assert_eq!(Lang::from_extension("bs"), Lang::BrightScript);
}
