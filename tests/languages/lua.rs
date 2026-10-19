use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
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
            sym("run", SymKind::Method, Some("M"), true),
            sym("helper", SymKind::Function, None, true),
        ],
        rejected_syms: &["M"],
        expected_edges: &[("run", "helper")],
        expect_external: true,
        ..Default::default()
    });
    assert!(Lang::Lua.semantics().is_std_or_builtin_member("insert"));
}
