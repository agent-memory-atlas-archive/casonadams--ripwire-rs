use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_lua_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
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
    });
    assert!(Lang::Lua.semantics().is_std_or_builtin_member("insert"));
}
